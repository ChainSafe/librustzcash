use std::{collections::BTreeMap, ops::Deref};

use zcash_primitives::transaction::{components::OutPoint, TxId};
use zcash_protocol::{memo::Memo, value::Zatoshis, PoolType, ShieldedProtocol::Sapling};

use zcash_client_backend::{
    data_api::{SentTransaction, SentTransactionOutput},
    wallet::{Note, NoteId, Recipient},
};

use crate::AccountId;

#[cfg(feature = "orchard")]
use zcash_protocol::ShieldedProtocol::Orchard;

#[derive(PartialEq, PartialOrd, Eq, Ord, Debug, Clone)]
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

#[derive(Debug, Clone, PartialEq)]
pub(crate) struct SentNoteTable(pub(crate) BTreeMap<SentNoteId, SentNote>);

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

#[derive(Debug, Clone, PartialEq)]
pub(crate) struct SentNote {
    pub(crate) from_account_id: AccountId,
    pub(crate) to: Recipient<AccountId, Note, OutPoint>,
    pub(crate) value: Zatoshis,
    pub(crate) memo: Memo,
}

mod serialization {
    use super::*;
    use crate::{error::Error, proto::memwallet as proto, read_optional};
    use zcash_address::ZcashAddress;
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

    impl TryFrom<proto::SentNote> for SentNote {
        type Error = crate::Error;

        fn try_from(note: proto::SentNote) -> Result<Self, Self::Error> {
            Ok(Self {
                from_account_id: note.from_account_id.into(),
                to: read_optional!(note, to)?.into(),
                value: Zatoshis::from_u64(note.value)?,
                memo: Memo::from_bytes(&note.memo)?,
            })
        }
    }

    impl From<SentNoteId> for proto::NoteId {
        fn from(note_id: SentNoteId) -> Self {
            match note_id {
                SentNoteId::Shielded(note_id) => proto::NoteId {
                    tx_id: Some(note_id.txid().into()),
                    output_index: note_id.output_index().into(),
                    pool: match note_id.protocol() {
                        Sapling => proto::PoolType::ShieldedSapling as i32,
                        #[cfg(feature = "orchard")]
                        Orchard => proto::PoolType::ShieldedOrchard as i32,
                    },
                },
                SentNoteId::Transparent { txid, output_index } => proto::NoteId {
                    tx_id: Some(txid.into()),
                    output_index: output_index.into(),
                    pool: proto::PoolType::Transparent as i32,
                },
            }
        }
    }

    impl From<proto::NoteId> for SentNoteId {
        fn from(note_id: proto::NoteId) -> Self {
            match note_id.pool() {
                proto::PoolType::ShieldedSapling => SentNoteId::Shielded(NoteId::new(
                    note_id.tx_id.unwrap().into(),
                    Sapling,
                    note_id.output_index.try_into().unwrap(),
                )),
                #[cfg(feature = "orchard")]
                proto::PoolType::ShieldedOrchard => SentNoteId::Shielded(NoteId::new(
                    note_id.tx_id.unwrap().into(),
                    Orchard,
                    note_id.output_index.try_into().unwrap(),
                )),
                proto::PoolType::Transparent => SentNoteId::Transparent {
                    txid: note_id.tx_id.unwrap().into(),
                    output_index: note_id.output_index.try_into().unwrap(),
                },
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
                    recipient_type: proto::RecipientType::EphemeralTransparent as i32,

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
                    recipient_type: proto::RecipientType::InternalAccount as i32,

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
            match recipient.recipient_type() {
                proto::RecipientType::ExternalRecipient => {
                    let address_str = recipient.address.clone().unwrap();
                    let address = ZcashAddress::try_from_encoded(&address_str);
                    Recipient::External(
                        address.unwrap(),
                        match recipient.pool_type() {
                            proto::PoolType::Transparent => PoolType::Transparent,
                            proto::PoolType::ShieldedSapling => PoolType::Shielded(Sapling),
                            #[cfg(feature = "orchard")]
                            proto::PoolType::ShieldedOrchard => PoolType::Shielded(Orchard),
                        },
                    )
                }
                proto::RecipientType::EphemeralTransparent => Recipient::EphemeralTransparent {
                    receiving_account: recipient.account_id.unwrap().into(),
                    ephemeral_address: TransparentAddress::decode(
                        &EncodingParams,
                        &recipient.address.unwrap(),
                    )
                    .unwrap(),
                    outpoint_metadata: recipient.outpoint_metadata.unwrap().into(),
                },
                proto::RecipientType::InternalAccount => Recipient::InternalAccount {
                    receiving_account: recipient.account_id.unwrap().into(),
                    external_address: recipient.address.map(|a| a.parse().unwrap()),
                    note: recipient.note.unwrap().into(),
                },
            }
        }
    }

    #[cfg(test)]
    mod tests {
        use super::*;
        use crate::proto::memwallet as proto;
        use zcash_primitives::transaction::components::OutPoint;
        use zcash_protocol::ShieldedProtocol;

        #[test]
        fn proto_roundtrip_recipient() {
            let recipient = Recipient::<AccountId, Note, OutPoint>::External(
                ZcashAddress::try_from_encoded("uregtest1a7mkafdn9c87xywjnyup65uker8tx3y72r9f6elcfm6uh263c9s6smcw6xm5m8k8eythcreuyqktp9z7mtpcd6jsm5xw7skgdcfjx84z").unwrap(),
                PoolType::Shielded(ShieldedProtocol::Sapling),
            );
            let proto = proto::Recipient::from(recipient.clone());
            let recipient2 = Recipient::<AccountId, Note, OutPoint>::from(proto.clone());
            let proto2 = proto::Recipient::from(recipient2.clone());
            assert_eq!(proto, proto2);
        }
    }
}
