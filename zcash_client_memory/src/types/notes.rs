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

/// Keeps track of notes that are spent in which transaction
#[derive(Debug)]
pub(crate) struct ReceievdNoteSpends(BTreeMap<NoteId, TxId>);

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
pub(crate) struct ReceivedNoteTable(Vec<ReceivedNote>);

#[derive(Debug, Clone)]
pub(crate) struct ReceivedNote {
    // Uniquely identifies this note
    pub(crate) note_id: NoteId,
    pub(crate) txid: TxId,
    // output_index: sapling, action_index: orchard
    pub(crate) output_index: u32,
    pub(crate) account_id: AccountId,
    //sapling: (diversifier, value, rcm) orchard: (diversifier, value, rho, rseed)
    pub(crate) note: Note,
    pub(crate) nf: Option<Nullifier>,
    pub(crate) is_change: bool,
    pub(crate) memo: Memo,
    pub(crate) commitment_tree_position: Option<Position>,
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
    use jubjub::Fr;

    use super::*;
    use crate::proto::memwallet as proto;

    impl From<Note> for proto::Note {
        fn from(note: Note) -> Self {
            match note {
                Note::Sapling(note) => Self {
                    protocol: proto::ShieldedProtocol::Sapling.into(),
                    recipient: note.recipient().to_bytes().to_vec(),
                    value: note.value().inner(),
                    rseed: match note.rseed() {
                        sapling::Rseed::AfterZip212(inner) => Some(proto::RSeed {
                            rseed_type: Some(proto::RSeedType::AfterZip212 as i32),
                            payload: inner.to_vec(),
                        }),
                        sapling::Rseed::BeforeZip212(inner) => Some(proto::RSeed {
                            rseed_type: Some(proto::RSeedType::BeforeZip212 as i32),
                            payload: inner.to_bytes().to_vec(),
                        }),
                    },
                    rho: None,
                },
                #[cfg(feature = "orchard")]
                Note::Orchard(note) => Self {
                    protocol: proto::ShieldedProtocol::Orchard.into(),
                    recipient: note.recipient().to_raw_address_bytes().to_vec(),
                    value: note.value().inner(),
                    rseed: Some(proto::RSeed {
                        rseed_type: None,
                        payload: note.rseed().as_bytes().to_vec(),
                    }),
                    rho: Some(note.rho().to_bytes().to_vec()),
                },
            }
        }
    }

    impl From<proto::Note> for Note {
        fn from(note: proto::Note) -> Self {
            match note.protocol {
                0 => {
                    let recipient =
                        sapling::PaymentAddress::from_bytes(&note.recipient.try_into().unwrap())
                            .unwrap();
                    let value = sapling::value::NoteValue::from_raw(note.value);
                    let rseed = match note.rseed {
                        Some(proto::RSeed {
                            rseed_type: Some(0),
                            payload,
                        }) => sapling::Rseed::BeforeZip212(
                            Fr::from_bytes(&payload.try_into().unwrap()).unwrap(),
                        ),
                        Some(proto::RSeed {
                            rseed_type: Some(1),
                            payload,
                        }) => sapling::Rseed::AfterZip212(payload.try_into().unwrap()),
                        _ => panic!("rseed is required"),
                    };
                    Self::Sapling(sapling::Note::from_parts(recipient, value, rseed))
                }
                1 => {
                    let recipient = orchard::Address::from_raw_address_bytes(
                        &note.recipient.try_into().unwrap(),
                    )
                    .unwrap();
                    let value = orchard::value::NoteValue::from_raw(note.value);
                    let rho =
                        orchard::note::Rho::from_bytes(&note.rho.unwrap().try_into().unwrap())
                            .unwrap();
                    let rseed = orchard::note::RandomSeed::from_bytes(
                        note.rseed.unwrap().payload.try_into().unwrap(),
                        &rho,
                    )
                    .unwrap();
                    Self::Orchard(orchard::Note::from_parts(recipient, value, rho, rseed).unwrap())
                }
                _ => panic!("invalid protocol"),
            }
        }
    }

    #[cfg(test)]
    mod tests {
        use super::*;
        use crate::proto::memwallet as proto;
        use pretty_assertions::assert_eq;

        #[test]
        fn test_note_roundtrip() {
            let note = Note::Sapling(sapling::note::Note::from_parts(
                sapling::PaymentAddress::from_bytes(&[
                    0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x30, 0x8e,
                    0x11, 0x9d, 0x72, 0x99, 0x2b, 0x56, 0x0d, 0x26, 0x50, 0xff, 0xe0, 0xbe, 0x7f,
                    0x35, 0x42, 0xfd, 0x97, 0x00, 0x3c, 0xb7, 0xcc, 0x3a, 0xbf, 0xf8, 0x1a, 0x7f,
                    0x90, 0x37, 0xf3, 0xea,
                ])
                .unwrap(),
                sapling::value::NoteValue::from_raw(99),
                sapling::Rseed::AfterZip212([0; 32]),
            ));

            let proto_note: proto::Note = note.clone().into();
            let recovered: Note = proto_note.into();

            assert_eq!(note, recovered);
        }
    }
}
