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
