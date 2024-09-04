use crate::serialization::*;
use incrementalmerkletree::Position;
use sapling::circuit::Spend;
use serde::{Deserialize, Serialize};
use serde_with::serde_as;
use serde_with::{FromInto, TryFromInto};
use std::{
    collections::HashMap,
    ops::{Deref, DerefMut},
};

use zip32::Scope;

use zcash_primitives::transaction::TxId;
use zcash_protocol::{memo::Memo, PoolType, ShieldedProtocol::Sapling};

use zcash_client_backend::{
    data_api::{SentTransactionOutput, SpendableNotes},
    wallet::{Note, NoteId, Recipient, WalletSaplingOutput},
};

use crate::AccountId;

#[cfg(feature = "orchard")]
use {
    zcash_client_backend::wallet::WalletOrchardOutput, zcash_protocol::ShieldedProtocol::Orchard,
};

use crate::{error::Error, Nullifier};

/// Keeps track of notes that are spent in which transaction
pub(crate) struct ReceievdNoteSpends(HashMap<NoteId, TxId>);

impl ReceievdNoteSpends {
    pub fn new() -> Self {
        Self(HashMap::new())
    }
    pub fn insert_spend(&mut self, note_id: NoteId, txid: TxId) -> Option<TxId> {
        self.0.insert(note_id, txid)
    }
    pub fn get(&self, note_id: &NoteId) -> Option<&TxId> {
        self.0.get(note_id)
    }
}

/// A note that has been received by the wallet
/// TODO: Instead of Vec, perhaps we should identify by some unique ID
pub(crate) struct ReceivedNoteTable(pub Vec<ReceivedNote>);

#[serde_as]
#[derive(Serialize, Deserialize)]
pub(crate) struct ReceivedNote {
    // Uniquely identifies this note
    #[serde_as(as = "NoteIdWrapper")]
    pub(crate) note_id: NoteId,
    #[serde_as(as = "TxIdWrapper")]
    pub(crate) txid: TxId,
    // output_index: sapling, action_index: orchard
    pub(crate) output_index: u32,
    pub(crate) account_id: AccountId,
    //sapling: (diversifier, value, rcm) orchard: (diversifier, value, rho, rseed)
    #[serde_as(as = "serialization::NoteWrapper")]
    pub(crate) note: Note,
    pub(crate) nf: Option<Nullifier>,
    pub(crate) _is_change: bool,
    #[serde_as(as = "MemoBytesWrapper")]
    pub(crate) memo: Memo,
    #[serde_as(as = "Option<FromInto<u64>>")]
    pub(crate) commitment_tree_position: Option<Position>,
    #[serde_as(as = "Option<serialization::ScopeWrapper>")]
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
                _is_change: true,
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
                _is_change: true,
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
            _is_change: output.is_change(),
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
            _is_change: output.is_change(),
            memo: Memo::Empty,
            commitment_tree_position: Some(output.note_commitment_tree_position()),
            recipient_key_scope: output.recipient_key_scope(),
        }
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
        self.0.push(note);
    }
}

impl IntoIterator for ReceivedNoteTable {
    type Item = ReceivedNote;
    type IntoIter = <Vec<Self::Item> as IntoIterator>::IntoIter;

    fn into_iter(self) -> Self::IntoIter {
        self.0.into_iter()
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
    received_notes: &[&ReceivedNote],
) -> Result<SpendableNotes<NoteId>, Error> {
    let mut sapling = Vec::new();
    #[cfg(feature = "orchard")]
    let mut orchard = Vec::new();

    for note in received_notes {
        match note.note.clone() {
            Note::Sapling(inner) => {
                sapling.push(zcash_client_backend::wallet::ReceivedNote::from_parts(
                    note.note_id,
                    note.txid(),
                    note.output_index.try_into().unwrap(), // this overflow can never happen or else the chain is broken
                    inner,
                    note.recipient_key_scope
                        .ok_or(Error::Missing("recipient key scope".into()))?,
                    note.commitment_tree_position
                        .ok_or(Error::Missing("commitment tree position".into()))?,
                ));
            }
            #[cfg(feature = "orchard")]
            Note::Orchard(inner) => {
                orchard.push(zcash_client_backend::wallet::ReceivedNote::from_parts(
                    note.note_id,
                    note.txid(),
                    note.output_index.try_into().unwrap(), // this overflow can never happen or else the chain is broken
                    inner,
                    note.recipient_key_scope
                        .ok_or(Error::Missing("recipient key scope".into()))?,
                    note.commitment_tree_position
                        .ok_or(Error::Missing("commitment tree position".into()))?,
                ));
            }
        }
    }

    Ok(SpendableNotes::new(
        sapling,
        #[cfg(feature = "orchard")]
        orchard,
    ))
}

mod serialization {
    use crate::types::serialization::arrays;
    use sapling::{value::NoteValue, PaymentAddress, Rseed};
    use serde::{
        de::VariantAccess,
        ser::{SerializeStruct, SerializeTupleVariant},
        Deserialize, Serialize,
    };
    use serde_with::{
        de::DeserializeAsWrap, ser::SerializeAsWrap, serde_as, DeserializeAs, SerializeAs,
    };
    use zcash_client_backend::wallet::Note;
    use zip32::Scope;

    #[derive(Serialize, Deserialize)]
    #[serde(remote = "Scope")]
    pub enum ScopeWrapper {
        External,
        Internal,
    }
    impl From<zip32::Scope> for ScopeWrapper {
        fn from(value: zip32::Scope) -> Self {
            match value {
                zip32::Scope::External => ScopeWrapper::External,
                zip32::Scope::Internal => ScopeWrapper::Internal,
            }
        }
    }
    impl serde_with::SerializeAs<Scope> for ScopeWrapper {
        fn serialize_as<S>(value: &Scope, serializer: S) -> Result<S::Ok, S::Error>
        where
            S: serde::Serializer,
        {
            ScopeWrapper::serialize(value, serializer)
        }
    }
    impl<'de> serde_with::DeserializeAs<'de, Scope> for ScopeWrapper {
        fn deserialize_as<D>(deserializer: D) -> Result<Scope, D::Error>
        where
            D: serde::Deserializer<'de>,
        {
            ScopeWrapper::deserialize(deserializer).map(Into::into)
        }
    }

    pub struct PaymentAddressWrapper;
    impl serde_with::SerializeAs<sapling::PaymentAddress> for PaymentAddressWrapper {
        fn serialize_as<S>(
            value: &sapling::PaymentAddress,
            serializer: S,
        ) -> Result<S::Ok, S::Error>
        where
            S: serde::Serializer,
        {
            value.to_bytes().serialize(serializer)
        }
    }
    impl<'de> serde_with::DeserializeAs<'de, sapling::PaymentAddress> for PaymentAddressWrapper {
        fn deserialize_as<D>(deserializer: D) -> Result<sapling::PaymentAddress, D::Error>
        where
            D: serde::Deserializer<'de>,
        {
            Ok(
                sapling::PaymentAddress::from_bytes(&arrays::deserialize::<_, u8, 43>(
                    deserializer,
                )?)
                .ok_or_else(|| serde::de::Error::custom("Invalid sapling payment address"))?,
            )
        }
    }
    #[cfg(feature = "orchard")]
    impl serde_with::SerializeAs<orchard::Address> for PaymentAddressWrapper {
        fn serialize_as<S>(value: &orchard::Address, serializer: S) -> Result<S::Ok, S::Error>
        where
            S: serde::Serializer,
        {
            value.to_raw_address_bytes().serialize(serializer)
        }
    }
    #[cfg(feature = "orchard")]
    impl<'de> serde_with::DeserializeAs<'de, orchard::Address> for PaymentAddressWrapper {
        fn deserialize_as<D>(deserializer: D) -> Result<orchard::Address, D::Error>
        where
            D: serde::Deserializer<'de>,
        {
            Ok(
                orchard::Address::from_raw_address_bytes(&arrays::deserialize::<_, u8, 43>(
                    deserializer,
                )?)
                .into_option()
                .ok_or_else(|| serde::de::Error::custom("Invalid orchard payment address"))?,
            )
        }
    }

    pub enum RseedWrapper {
        BeforeZip212(jubjub::Fr),
        AfterZip212([u8; 32]),
    }
    impl From<RseedWrapper> for Rseed {
        fn from(def: RseedWrapper) -> Rseed {
            match def {
                RseedWrapper::BeforeZip212(rcm) => Rseed::BeforeZip212(rcm),
                RseedWrapper::AfterZip212(rseed) => Rseed::AfterZip212(rseed),
            }
        }
    }
    impl serde_with::SerializeAs<Rseed> for RseedWrapper {
        fn serialize_as<S>(value: &Rseed, serializer: S) -> Result<S::Ok, S::Error>
        where
            S: serde::Serializer,
        {
            match value {
                Rseed::BeforeZip212(rcm) => serializer.serialize_newtype_variant(
                    "Rseed",
                    0,
                    "BeforeZip212",
                    &rcm.to_bytes(),
                ),
                Rseed::AfterZip212(rseed) => {
                    serializer.serialize_newtype_variant("Rseed", 1, "AfterZip212", rseed)
                }
            }
        }
    }

    impl<'de> serde_with::DeserializeAs<'de, Rseed> for RseedWrapper {
        fn deserialize_as<D>(deserializer: D) -> Result<Rseed, D::Error>
        where
            D: serde::Deserializer<'de>,
        {
            #[derive(Deserialize)]
            enum RseedDiscriminant {
                BeforeZip212,
                AfterZip212,
            }
            struct Visitor;
            impl<'de> serde::de::Visitor<'de> for Visitor {
                type Value = Rseed;
                fn expecting(&self, formatter: &mut std::fmt::Formatter) -> std::fmt::Result {
                    formatter.write_str("enum Rseed")
                }
                fn visit_enum<A>(self, data: A) -> Result<Rseed, A::Error>
                where
                    A: serde::de::EnumAccess<'de>,
                {
                    match data.variant()? {
                        (RseedDiscriminant::BeforeZip212, v) => Ok(RseedWrapper::BeforeZip212(
                            jubjub::Fr::from_bytes(&v.newtype_variant::<[u8; 32]>()?)
                                .into_option()
                                .ok_or_else(|| serde::de::Error::custom("Invalid Rseed"))?,
                        )
                        .into()),
                        (RseedDiscriminant::AfterZip212, v) => {
                            Ok(RseedWrapper::AfterZip212(v.newtype_variant::<[u8; 32]>()?).into())
                        }
                    }
                }
            }
            deserializer.deserialize_enum("Rseed", &["BeforeZip212", "AfterZip212"], Visitor)
        }
    }

    pub struct NoteValueWrapper;
    impl serde_with::SerializeAs<sapling::value::NoteValue> for NoteValueWrapper {
        fn serialize_as<S>(
            value: &sapling::value::NoteValue,
            serializer: S,
        ) -> Result<S::Ok, S::Error>
        where
            S: serde::Serializer,
        {
            value.inner().serialize(serializer)
        }
    }
    impl<'de> serde_with::DeserializeAs<'de, sapling::value::NoteValue> for NoteValueWrapper {
        fn deserialize_as<D>(deserializer: D) -> Result<sapling::value::NoteValue, D::Error>
        where
            D: serde::Deserializer<'de>,
        {
            Ok(sapling::value::NoteValue::from_raw(u64::deserialize(
                deserializer,
            )?))
        }
    }
    #[cfg(feature = "orchard")]
    impl serde_with::SerializeAs<orchard::value::NoteValue> for NoteValueWrapper {
        fn serialize_as<S>(
            value: &orchard::value::NoteValue,
            serializer: S,
        ) -> Result<S::Ok, S::Error>
        where
            S: serde::Serializer,
        {
            value.inner().serialize(serializer)
        }
    }
    #[cfg(feature = "orchard")]
    impl<'de> serde_with::DeserializeAs<'de, orchard::value::NoteValue> for NoteValueWrapper {
        fn deserialize_as<D>(deserializer: D) -> Result<orchard::value::NoteValue, D::Error>
        where
            D: serde::Deserializer<'de>,
        {
            Ok(orchard::value::NoteValue::from_raw(u64::deserialize(
                deserializer,
            )?))
        }
    }

    #[cfg(feature = "orchard")]
    pub struct RhoWrapper;
    #[cfg(feature = "orchard")]
    impl serde_with::SerializeAs<orchard::note::Rho> for RhoWrapper {
        fn serialize_as<S>(value: &orchard::note::Rho, serializer: S) -> Result<S::Ok, S::Error>
        where
            S: serde::Serializer,
        {
            value.to_bytes().serialize(serializer)
        }
    }
    #[cfg(feature = "orchard")]
    impl<'de> serde_with::DeserializeAs<'de, orchard::note::Rho> for RhoWrapper {
        fn deserialize_as<D>(deserializer: D) -> Result<orchard::note::Rho, D::Error>
        where
            D: serde::Deserializer<'de>,
        {
            Ok(
                orchard::note::Rho::from_bytes(&<[u8; 32]>::deserialize(deserializer)?)
                    .into_option()
                    .ok_or_else(|| serde::de::Error::custom("Invalid rho"))?,
            )
        }
    }

    #[serde_as]
    #[derive(Serialize, Deserialize)]
    #[serde(remote = "sapling::Note")]
    pub struct SaplingNoteWrapper {
        /// The recipient of the funds.
        #[serde_as(as = "PaymentAddressWrapper")]
        #[serde(getter = "sapling::Note::recipient")]
        recipient: PaymentAddress,
        /// The value of this note.
        #[serde_as(as = "NoteValueWrapper")]
        #[serde(getter = "sapling::Note::value")]
        value: NoteValue,
        /// The seed randomness for various note components.
        #[serde(getter = "sapling::Note::rseed")]
        #[serde_as(as = "RseedWrapper")]
        rseed: Rseed,
    }
    impl From<SaplingNoteWrapper> for sapling::Note {
        fn from(note: SaplingNoteWrapper) -> Self {
            sapling::Note::from_parts(note.recipient, note.value, note.rseed)
        }
    }
    impl serde_with::SerializeAs<sapling::Note> for SaplingNoteWrapper {
        fn serialize_as<S>(value: &sapling::Note, serializer: S) -> Result<S::Ok, S::Error>
        where
            S: serde::Serializer,
        {
            SaplingNoteWrapper::serialize(value, serializer)
        }
    }
    impl<'de> serde_with::DeserializeAs<'de, sapling::Note> for SaplingNoteWrapper {
        fn deserialize_as<D>(deserializer: D) -> Result<sapling::Note, D::Error>
        where
            D: serde::Deserializer<'de>,
        {
            SaplingNoteWrapper::deserialize(deserializer).map(Into::into)
        }
    }

    #[cfg(feature = "orchard")]
    #[serde_as]
    pub struct OrchardNoteWrapper {
        // #[serde_as(as = "PaymentAddressWrapper")]
        recipient: orchard::Address,
        // #[serde_as(as = "NoteValueWrapper")]
        value: orchard::value::NoteValue,
        // #[serde_as(as = "RhoWrapper")]
        rho: orchard::note::Rho,
        // #[serde_as(as = "RseedWrapper")]
        rseed: orchard::note::RandomSeed,
    }

    #[cfg(feature = "orchard")]
    impl serde_with::SerializeAs<orchard::note::Note> for OrchardNoteWrapper {
        fn serialize_as<S>(value: &orchard::note::Note, serializer: S) -> Result<S::Ok, S::Error>
        where
            S: serde::Serializer,
        {
            let mut s = serializer.serialize_struct("OrchardNote", 4)?;
            s.serialize_field(
                "recipient",
                &SerializeAsWrap::<_, PaymentAddressWrapper>::new(&value.recipient()),
            )?;
            s.serialize_field(
                "value",
                &SerializeAsWrap::<_, NoteValueWrapper>::new(&value.value()),
            )?;
            s.serialize_field("rho", &SerializeAsWrap::<_, RhoWrapper>::new(&value.rho()))?;
            s.serialize_field("rseed", value.rseed().as_bytes())?;
            s.end()
        }
    }
    #[cfg(feature = "orchard")]
    impl<'de> serde_with::DeserializeAs<'de, orchard::note::Note> for OrchardNoteWrapper {
        fn deserialize_as<D>(deserializer: D) -> Result<orchard::note::Note, D::Error>
        where
            D: serde::Deserializer<'de>,
        {
            struct Visitor;
            impl<'de> serde::de::Visitor<'de> for Visitor {
                type Value = orchard::note::Note;
                fn expecting(&self, formatter: &mut std::fmt::Formatter) -> std::fmt::Result {
                    formatter.write_str("struct OrchardNote")
                }
                fn visit_seq<A>(self, mut seq: A) -> Result<orchard::note::Note, A::Error>
                where
                    A: serde::de::SeqAccess<'de>,
                {
                    let recipient = seq
                        .next_element::<DeserializeAsWrap<orchard::Address, PaymentAddressWrapper>>(
                        )?
                        .ok_or_else(|| serde::de::Error::invalid_length(0, &"a recipient"))?
                        .into_inner();
                    let value = seq
                        .next_element::<DeserializeAsWrap<orchard::value::NoteValue, NoteValueWrapper>>()?
                        .ok_or_else(|| serde::de::Error::invalid_length(1, &"a value"))?
                        .into_inner();
                    let rho = seq
                        .next_element::<DeserializeAsWrap<orchard::note::Rho, RhoWrapper>>()?
                        .ok_or_else(|| serde::de::Error::invalid_length(2, &"a rho"))?
                        .into_inner();
                    let rseed = seq
                        .next_element::<[u8; 32]>()?
                        .ok_or_else(|| serde::de::Error::invalid_length(3, &"an rseed"))?;
                    let rseed = orchard::note::RandomSeed::from_bytes(rseed, &rho)
                        .into_option()
                        .ok_or_else(|| serde::de::Error::custom("Invalid rseed"))?;
                    Ok(
                        orchard::note::Note::from_parts(recipient, value, rho, rseed)
                            .into_option()
                            .ok_or_else(|| serde::de::Error::custom("Invalid orchard note"))?,
                    )
                }
                fn visit_map<A>(self, mut map: A) -> Result<orchard::note::Note, A::Error>
                where
                    A: serde::de::MapAccess<'de>,
                {
                    let mut recipient = None;
                    let mut value = None;
                    let mut rho = None;
                    let mut rseed = None;
                    while let Some(key) = map.next_key()? {
                        match key {
                            "recipient" => {
                                recipient =
                                    Some(map.next_value::<DeserializeAsWrap<
                                        orchard::Address,
                                        PaymentAddressWrapper,
                                    >>()?);
                            }
                            "value" => {
                                value = Some(map.next_value::<DeserializeAsWrap<
                                    orchard::value::NoteValue,
                                    NoteValueWrapper,
                                >>()?);
                            }
                            "rho" => {
                                rho = Some(map.next_value::<DeserializeAsWrap<orchard::note::Rho, RhoWrapper>>()?);
                            }
                            "rseed" => {
                                rseed = Some(map.next_value::<[u8; 32]>()?);
                            }
                            _ => {
                                return Err(serde::de::Error::unknown_field(
                                    key,
                                    &["recipient", "value", "rho", "rseed"],
                                ));
                            }
                        }
                    }
                    let recipient = recipient
                        .ok_or_else(|| serde::de::Error::missing_field("recipient"))?
                        .into_inner();
                    let value = value
                        .ok_or_else(|| serde::de::Error::missing_field("value"))?
                        .into_inner();
                    let rho = rho
                        .ok_or_else(|| serde::de::Error::missing_field("rho"))?
                        .into_inner();
                    let rseed = rseed.ok_or_else(|| serde::de::Error::missing_field("rseed"))?;
                    let rseed = orchard::note::RandomSeed::from_bytes(rseed, &rho)
                        .into_option()
                        .ok_or_else(|| serde::de::Error::custom("Invalid rseed"))?;
                    Ok(
                        orchard::note::Note::from_parts(recipient, value, rho, rseed)
                            .into_option()
                            .ok_or_else(|| serde::de::Error::custom("Invalid orchard note"))?,
                    )
                }
            }
            deserializer.deserialize_struct(
                "OrchardNote",
                &["recipient", "value", "rho", "rseed"],
                Visitor,
            )
        }
    }

    #[serde_as]
    #[derive(Serialize, Deserialize)]
    #[serde(remote = "Note")]
    pub enum NoteWrapper {
        Sapling(#[serde_as(as = "SaplingNoteWrapper")] sapling::Note),
        #[cfg(feature = "orchard")]
        Orchard(#[serde_as(as = "OrchardNoteWrapper")] orchard::Note),
    }

    impl From<NoteWrapper> for Note {
        fn from(note: NoteWrapper) -> Self {
            match note {
                NoteWrapper::Sapling(inner) => Note::Sapling(inner),
                #[cfg(feature = "orchard")]
                NoteWrapper::Orchard(inner) => Note::Orchard(inner),
            }
        }
    }

    impl SerializeAs<Note> for NoteWrapper {
        fn serialize_as<S>(value: &Note, serializer: S) -> Result<S::Ok, S::Error>
        where
            S: serde::Serializer,
        {
            NoteWrapper::serialize(value, serializer)
        }
    }
    impl<'de> DeserializeAs<'de, Note> for NoteWrapper {
        fn deserialize_as<D>(deserializer: D) -> Result<Note, D::Error>
        where
            D: serde::Deserializer<'de>,
        {
            NoteWrapper::deserialize(deserializer).map(Into::into)
        }
    }
}
