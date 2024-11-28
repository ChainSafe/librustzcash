use incrementalmerkletree::Position;

use std::collections::BTreeSet;
use std::{
    collections::BTreeMap,
    ops::{Deref, DerefMut},
};
use zip32::Scope;

use zcash_primitives::transaction::{components::OutPoint, TxId};
use zcash_protocol::{memo::Memo, value::Zatoshis, PoolType, ShieldedProtocol::Sapling};

use zcash_client_backend::{
    data_api::{SentTransaction, SentTransactionOutput, SpendableNotes},
    wallet::{Note, NoteId, Recipient, WalletSaplingOutput},
};

use crate::AccountId;

#[cfg(feature = "orchard")]
use {
    zcash_client_backend::wallet::WalletOrchardOutput, zcash_protocol::ShieldedProtocol::Orchard,
};

use crate::{error::Error, Nullifier};

#[derive(PartialEq, PartialOrd, Eq, Ord, Debug)]
pub enum SentNoteId {
    Shielded(NoteId),
    Transparent { txid: TxId, output_index: u32 },
}

impl From<NoteId> for SentNoteId {
    fn from(note_id: NoteId) -> Self {
        SentNoteId::Shielded(note_id)
    }
}

impl From<&NoteId> for SentNoteId {
    fn from(note_id: &NoteId) -> Self {
        SentNoteId::Shielded(*note_id)
    }
}

impl SentNoteId {
    pub fn txid(&self) -> &TxId {
        match self {
            SentNoteId::Shielded(note_id) => note_id.txid(),
            SentNoteId::Transparent { txid, .. } => txid,
        }
    }
}

pub(crate) struct SentNoteTable(BTreeMap<SentNoteId, SentNote>);

impl SentNoteTable {
    pub fn new() -> Self {
        Self(BTreeMap::new())
    }

    pub fn insert_sent_output(
        &mut self,
        tx: &SentTransaction<AccountId>,
        output: &SentTransactionOutput<AccountId>,
    ) {
        let pool_type = match output.recipient() {
            Recipient::External(_, pool_type) => *pool_type,
            Recipient::EphemeralTransparent { .. } => PoolType::Transparent,
            Recipient::InternalAccount { note, .. } => PoolType::Shielded(note.protocol()),
        };
        match pool_type {
            PoolType::Transparent => {
                // we kind of are in a tricky spot here since NoteId cannot represent a transparent note..
                // just make it a sapling one for now until we figure out a better way to represent this
                let note_id = SentNoteId::Transparent {
                    txid: tx.tx().txid(),
                    output_index: output.output_index().try_into().unwrap(),
                };
                self.0.insert(
                    note_id,
                    SentNote {
                        from_account_id: *tx.account_id(),
                        to: output.recipient().clone(),
                        value: output.value(),
                        memo: Memo::Empty, // transparent notes don't have memos
                    },
                );
            }
            PoolType::Shielded(protocol) => {
                let note_id = NoteId::new(
                    tx.tx().txid(),
                    protocol,
                    output.output_index().try_into().unwrap(),
                );
                self.0.insert(
                    note_id.into(),
                    SentNote {
                        from_account_id: *tx.account_id(),
                        to: output.recipient().clone(),
                        value: output.value(),
                        memo: output.memo().map(|m| Memo::try_from(m).unwrap()).unwrap(),
                    },
                );
            }
        }
    }

    pub fn put_sent_output(
        &mut self,
        txid: TxId,
        from_account_id: AccountId,
        output: &SentTransactionOutput<AccountId>,
    ) {
        let pool_type = match output.recipient() {
            Recipient::External(_, pool_type) => *pool_type,
            Recipient::EphemeralTransparent { .. } => PoolType::Transparent,
            Recipient::InternalAccount { note, .. } => PoolType::Shielded(note.protocol()),
        };
        match pool_type {
            PoolType::Transparent => {
                // we kind of are in a tricky spot here since NoteId cannot represent a transparent note..
                // just make it a sapling one for now until we figure out a better way to represent this
                let note_id = SentNoteId::Transparent {
                    txid,
                    output_index: output.output_index().try_into().unwrap(),
                };
                self.0.insert(
                    note_id,
                    SentNote {
                        from_account_id,
                        to: output.recipient().clone(),
                        value: output.value(),
                        memo: Memo::Empty, // transparent notes don't have memos
                    },
                );
            }
            PoolType::Shielded(protocol) => {
                let note_id =
                    NoteId::new(txid, protocol, output.output_index().try_into().unwrap());
                self.0.insert(
                    note_id.into(),
                    SentNote {
                        from_account_id,
                        to: output.recipient().clone(),
                        value: output.value(),
                        memo: output.memo().map(|m| Memo::try_from(m).unwrap()).unwrap(),
                    },
                );
            }
        }
    }

    pub fn get_sent_note(&self, note_id: &NoteId) -> Option<&SentNote> {
        self.0.get(&note_id.into())
    }
}

impl Deref for SentNoteTable {
    type Target = BTreeMap<SentNoteId, SentNote>;

    fn deref(&self) -> &Self::Target {
        &self.0
    }
}

#[derive(Debug)]
pub(crate) struct SentNote {
    pub(crate) from_account_id: AccountId,
    pub(crate) to: Recipient<AccountId, Note, OutPoint>,
    pub(crate) value: Zatoshis,
    pub(crate) memo: Memo,
}

mod serialization {
    use super::*;
    use crate::proto::memwallet as proto;
    use zcash_keys::encoding::AddressCodec;
    use zcash_primitives::{
        consensus::Network::MainNetwork as EncodingParams, legacy::TransparentAddress,
    };

    impl From<SentNote> for proto::SentNote {
        fn from(note: SentNote) -> Self {
            Self {
                from_account_id: *note.from_account_id,
                to: Some(note.to.into()),
                value: note.value.into(),
                memo: note.memo.encode().as_array().to_vec(),
            }
        }
    }

    impl From<proto::SentNote> for SentNote {
        fn from(note: proto::SentNote) -> Self {
            Self {
                from_account_id: note.from_account_id.into(),
                to: note.to.unwrap().into(),
                value: Zatoshis::from_u64(note.value).unwrap(),
                memo: Memo::from_bytes(&note.memo).unwrap(),
            }
        }
    }

    impl From<OutPoint> for proto::OutPoint {
        fn from(outpoint: OutPoint) -> Self {
            Self {
                hash: outpoint.txid().as_ref().to_vec(),
                n: outpoint.n(),
            }
        }
    }

    impl From<proto::OutPoint> for OutPoint {
        fn from(outpoint: proto::OutPoint) -> Self {
            Self::new(outpoint.hash.try_into().unwrap(), outpoint.n)
        }
    }

    impl From<Recipient<AccountId, Note, OutPoint>> for proto::Recipient {
        fn from(recipient: Recipient<AccountId, Note, OutPoint>) -> Self {
            match recipient {
                Recipient::External(address, pool_type) => proto::Recipient {
                    recipient_type: proto::RecipientType::ExternalRecipient as i32,

                    address: Some(address.to_string()),
                    pool_type: Some(match pool_type {
                        PoolType::Transparent => proto::PoolType::Transparent,
                        PoolType::Shielded(Sapling) => proto::PoolType::ShieldedSapling,
                        #[cfg(feature = "orchard")]
                        PoolType::Shielded(Orchard) => proto::PoolType::ShieldedOrchard,
                    } as i32),

                    account_id: None,
                    outpoint_metadata: None,
                    note: None,
                },
                Recipient::EphemeralTransparent {
                    receiving_account,
                    ephemeral_address,
                    outpoint_metadata,
                } => proto::Recipient {
                    recipient_type: proto::RecipientType::ExternalRecipient as i32,

                    address: Some(ephemeral_address.encode(&EncodingParams)),
                    pool_type: Some(proto::PoolType::Transparent as i32),

                    account_id: Some(*receiving_account),
                    outpoint_metadata: Some(outpoint_metadata.into()),
                    note: None,
                },
                Recipient::InternalAccount {
                    receiving_account,
                    external_address,
                    note,
                } => proto::Recipient {
                    recipient_type: proto::RecipientType::ExternalRecipient as i32,

                    address: external_address.map(|a| a.to_string()),
                    pool_type: None,

                    account_id: Some(*receiving_account),
                    outpoint_metadata: None,
                    note: Some(note.into()),
                },
            }
        }
    }

    impl From<proto::Recipient> for Recipient<AccountId, Note, OutPoint> {
        fn from(recipient: proto::Recipient) -> Self {
            match recipient.recipient_type {
                0 => Recipient::External(
                    recipient.address.unwrap().parse().unwrap(),
                    match recipient.pool_type.unwrap() {
                        0 => PoolType::Transparent,
                        1 => PoolType::Shielded(Sapling),
                        #[cfg(feature = "orchard")]
                        2 => PoolType::Shielded(Orchard),
                        _ => unreachable!(),
                    },
                ),
                1 => Recipient::EphemeralTransparent {
                    receiving_account: recipient.account_id.unwrap().into(),
                    ephemeral_address: TransparentAddress::decode(
                        &EncodingParams,
                        &recipient.address.unwrap(),
                    )
                    .unwrap(),
                    outpoint_metadata: recipient.outpoint_metadata.unwrap().into(),
                },
                _ => Recipient::InternalAccount {
                    receiving_account: recipient.account_id.unwrap().into(),
                    external_address: recipient.address.map(|a| a.parse().unwrap()),
                    note: recipient.note.unwrap().into(),
                },
            }
        }
    }
}
