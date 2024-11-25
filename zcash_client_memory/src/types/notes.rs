use crate::serialization::*;
use incrementalmerkletree::Position;

use serde::{Deserialize, Serialize};
use serde_with::serde_as;
use serde_with::{FromInto, TryFromInto};

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

/// Keeps track of notes that are spent in which transaction
#[serde_as]
#[derive(Debug, Serialize, Deserialize)]
pub(crate) struct ReceievdNoteSpends(
    #[serde_as(as = "BTreeMap<NoteIdDef, ByteArray<32>>")] BTreeMap<NoteId, TxId>,
);

impl ReceievdNoteSpends {
    pub fn new() -> Self {
        Self(BTreeMap::new())
    }
    pub fn insert_spend(&mut self, note_id: NoteId, txid: TxId) -> Option<TxId> {
        self.0.insert(note_id, txid)
    }
    pub fn get(&self, note_id: &NoteId) -> Option<&TxId> {
        self.0.get(note_id)
    }
}

impl Deref for ReceievdNoteSpends {
    type Target = BTreeMap<NoteId, TxId>;

    fn deref(&self) -> &Self::Target {
        &self.0
    }
}

/// A note that has been received by the wallet
/// TODO: Instead of Vec, perhaps we should identify by some unique ID
#[derive(Serialize, Deserialize)]
pub(crate) struct ReceivedNoteTable(Vec<ReceivedNote>);

#[serde_as]
#[derive(Serialize, Deserialize, Debug, Clone)]
pub(crate) struct ReceivedNote {
    // Uniquely identifies this note
    #[serde_as(as = "NoteIdDef")]
    pub(crate) note_id: NoteId,
    #[serde_as(as = "ByteArray<32>")]
    pub(crate) txid: TxId,
    // output_index: sapling, action_index: orchard
    pub(crate) output_index: u32,
    pub(crate) account_id: AccountId,
    //sapling: (diversifier, value, rcm) orchard: (diversifier, value, rho, rseed)
    #[serde_as(as = "NoteDef")]
    pub(crate) note: Note,
    pub(crate) nf: Option<Nullifier>,
    pub(crate) is_change: bool,
    #[serde_as(as = "MemoBytesDef")]
    pub(crate) memo: Memo,
    #[serde_as(as = "Option<FromInto<u64>>")]
    pub(crate) commitment_tree_position: Option<Position>,
    #[serde_as(as = "Option<ScopeDef>")]
    pub(crate) recipient_key_scope: Option<Scope>,
}
impl ReceivedNote {
    pub fn pool(&self) -> PoolType {
        match self.note {
            Note::Sapling { .. } => PoolType::SAPLING,
            #[cfg(feature = "orchard")]
            Note::Orchard { .. } => PoolType::ORCHARD,
        }
    }
    pub fn account_id(&self) -> AccountId {
        self.account_id
    }
    pub fn nullifier(&self) -> Option<&Nullifier> {
        self.nf.as_ref()
    }
    pub fn txid(&self) -> TxId {
        self.txid
    }
    pub fn note_id(&self) -> NoteId {
        self.note_id
    }
    pub fn from_sent_tx_output(
        txid: TxId,
        output: &SentTransactionOutput<AccountId>,
    ) -> Result<Self, Error> {
        match output.recipient() {
            Recipient::InternalAccount {
                receiving_account,
                note: Note::Sapling(note),
                ..
            } => Ok(ReceivedNote {
                note_id: NoteId::new(txid, Sapling, output.output_index() as u16),
                txid,
                output_index: output.output_index() as u32,
                account_id: *receiving_account,
                note: Note::Sapling(note.clone()),
                nf: None,
                is_change: true,
                memo: output.memo().map(|m| Memo::try_from(m).unwrap()).unwrap(),
                commitment_tree_position: None,
                recipient_key_scope: Some(Scope::Internal),
            }),
            #[cfg(feature = "orchard")]
            Recipient::InternalAccount {
                receiving_account,
                note: Note::Orchard(note),
                ..
            } => Ok(ReceivedNote {
                note_id: NoteId::new(txid, Orchard, output.output_index() as u16),
                txid,
                output_index: output.output_index() as u32,
                account_id: *receiving_account,
                note: Note::Orchard(*note),
                nf: None,
                is_change: true,
                memo: output.memo().map(|m| Memo::try_from(m).unwrap()).unwrap(),
                commitment_tree_position: None,
                recipient_key_scope: Some(Scope::Internal),
            }),
            _ => Err(Error::Other(
                "Recipient is not an internal shielded account".to_owned(),
            )),
        }
    }
    pub fn from_wallet_sapling_output(
        note_id: NoteId,
        output: &WalletSaplingOutput<AccountId>,
    ) -> Self {
        ReceivedNote {
            note_id,
            txid: *note_id.txid(),
            output_index: output.index() as u32,
            account_id: *output.account_id(),
            note: Note::Sapling(output.note().clone()),
            nf: output.nf().map(|nf| Nullifier::Sapling(*nf)),
            is_change: output.is_change(),
            memo: Memo::Empty,
            commitment_tree_position: Some(output.note_commitment_tree_position()),
            recipient_key_scope: output.recipient_key_scope(),
        }
    }
    #[cfg(feature = "orchard")]
    pub fn from_wallet_orchard_output(
        note_id: NoteId,
        output: &WalletOrchardOutput<AccountId>,
    ) -> Self {
        ReceivedNote {
            note_id,
            txid: *note_id.txid(),
            output_index: output.index() as u32,
            account_id: *output.account_id(),
            note: Note::Orchard(*output.note()),
            nf: output.nf().map(|nf| Nullifier::Orchard(*nf)),
            is_change: output.is_change(),
            memo: Memo::Empty,
            commitment_tree_position: Some(output.note_commitment_tree_position()),
            recipient_key_scope: output.recipient_key_scope(),
        }
    }
}

impl From<ReceivedNote>
    for zcash_client_backend::wallet::ReceivedNote<NoteId, zcash_client_backend::wallet::Note>
{
    fn from(value: ReceivedNote) -> Self {
        zcash_client_backend::wallet::ReceivedNote::from_parts(
            value.note_id,
            value.txid,
            value.output_index.try_into().unwrap(),
            value.note,
            value.recipient_key_scope.unwrap(),
            value.commitment_tree_position.unwrap(),
        )
    }
}

impl ReceivedNoteTable {
    pub fn new() -> Self {
        Self(Vec::new())
    }

    pub fn get_sapling_nullifiers(
        &self,
    ) -> impl Iterator<Item = (AccountId, TxId, sapling::Nullifier)> + '_ {
        self.0.iter().filter_map(|entry| {
            if let Some(Nullifier::Sapling(nf)) = entry.nullifier() {
                Some((entry.account_id(), entry.txid(), *nf))
            } else {
                None
            }
        })
    }
    #[cfg(feature = "orchard")]
    pub fn get_orchard_nullifiers(
        &self,
    ) -> impl Iterator<Item = (AccountId, TxId, orchard::note::Nullifier)> + '_ {
        self.0.iter().filter_map(|entry| {
            if let Some(Nullifier::Orchard(nf)) = entry.nullifier() {
                Some((entry.account_id(), entry.txid(), *nf))
            } else {
                None
            }
        })
    }

    pub fn insert_received_note(&mut self, note: ReceivedNote) {
        // ensure note_id is unique.
        // follow upsert rules to update the note if it already exists
        if self
            .0
            .iter_mut()
            .find(|n| n.note_id == note.note_id)
            .map(|n| {
                n.nf = note.nf.or(n.nf);
                n.is_change = note.is_change || n.is_change;
                n.commitment_tree_position =
                    note.commitment_tree_position.or(n.commitment_tree_position);
            })
            .is_none()
        {
            self.0.push(note);
        }
    }

    #[cfg(feature = "orchard")]
    pub fn detect_orchard_spending_accounts<'a>(
        &self,
        nfs: impl Iterator<Item = &'a orchard::note::Nullifier>,
    ) -> Result<BTreeSet<AccountId>, Error> {
        let mut acc = BTreeSet::new();
        let nfs = nfs.collect::<Vec<_>>();
        for (nf, id) in self.0.iter().filter_map(|n| match (n.nf, n.account_id) {
            (Some(Nullifier::Orchard(nf)), account_id) => Some((nf, account_id)),
            _ => None,
        }) {
            if nfs.contains(&&nf) {
                acc.insert(id);
            }
        }
        Ok(acc)
    }

    pub fn detect_sapling_spending_accounts<'a>(
        &self,
        nfs: impl Iterator<Item = &'a sapling::Nullifier>,
    ) -> Result<BTreeSet<AccountId>, Error> {
        let mut acc = BTreeSet::new();
        let nfs = nfs.collect::<Vec<_>>();
        for (nf, id) in self.0.iter().filter_map(|n| match (n.nf, n.account_id) {
            (Some(Nullifier::Sapling(nf)), account_id) => Some((nf, account_id)),
            _ => None,
        }) {
            if nfs.contains(&&nf) {
                acc.insert(id);
            }
        }
        Ok(acc)
    }
}

// We deref to slice so that we can reuse the slice impls
impl Deref for ReceivedNoteTable {
    type Target = [ReceivedNote];

    fn deref(&self) -> &Self::Target {
        &self.0[..]
    }
}
impl DerefMut for ReceivedNoteTable {
    fn deref_mut(&mut self) -> &mut Self::Target {
        &mut self.0[..]
    }
}

pub(crate) fn to_spendable_notes(
    sapling_received_notes: &[&ReceivedNote],
    #[cfg(feature = "orchard")] orchard_received_notes: &[&ReceivedNote],
) -> Result<SpendableNotes<NoteId>, Error> {
    let sapling = sapling_received_notes
        .iter()
        .map(|note| {
            if let Note::Sapling(inner) = &note.note {
                Ok(zcash_client_backend::wallet::ReceivedNote::from_parts(
                    note.note_id,
                    note.txid(),
                    note.output_index.try_into().unwrap(), // this overflow can never happen or else the chain is broken
                    inner.clone(),
                    note.recipient_key_scope
                        .ok_or(Error::Missing("recipient key scope".into()))?,
                    note.commitment_tree_position
                        .ok_or(Error::Missing("commitment tree position".into()))?,
                ))
            } else {
                Err(Error::Other("Note is not a sapling note".to_owned()))
            }
        })
        .collect::<Result<Vec<_>, _>>()?;

    #[cfg(feature = "orchard")]
    let orchard = orchard_received_notes
        .iter()
        .map(|note| {
            if let Note::Orchard(inner) = &note.note {
                Ok(zcash_client_backend::wallet::ReceivedNote::from_parts(
                    note.note_id,
                    note.txid(),
                    note.output_index.try_into().unwrap(), // this overflow can never happen or else the chain is broken
                    *inner,
                    note.recipient_key_scope
                        .ok_or(Error::Missing("recipient key scope".into()))?,
                    note.commitment_tree_position
                        .ok_or(Error::Missing("commitment tree position".into()))?,
                ))
            } else {
                Err(Error::Other("Note is not an orchard note".to_owned()))
            }
        })
        .collect::<Result<Vec<_>, _>>()?;

    Ok(SpendableNotes::new(
        sapling,
        #[cfg(feature = "orchard")]
        orchard,
    ))
}

#[serde_as]
#[derive(PartialEq, PartialOrd, Eq, Ord, Serialize, Deserialize, Debug)]
pub enum SentNoteId {
    Shielded(#[serde_as(as = "NoteIdDef")] NoteId),
    Transparent {
        #[serde_as(as = "ByteArray<32>")]
        txid: TxId,
        output_index: u32,
    },
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

#[derive(Serialize, Deserialize)]
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

#[serde_as]
#[derive(Serialize, Deserialize, Debug)]
pub(crate) struct SentNote {
    pub(crate) from_account_id: AccountId,
    #[serde_as(as = "RecipientDef<AccountId, Note, OutPoint>")]
    pub(crate) to: Recipient<AccountId, Note, OutPoint>,
    #[serde_as(as = "TryFromInto<u64>")]
    pub(crate) value: Zatoshis,
    #[serde_as(as = "MemoBytesDef")]
    pub(crate) memo: Memo,
}
