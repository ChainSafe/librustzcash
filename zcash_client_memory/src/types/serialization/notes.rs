use sapling::{value::NoteValue, PaymentAddress, Rseed};
use serde_with::{DeserializeAs, SerializeAs};

use zcash_client_backend::wallet::Note;
use zip32::Scope;

use std::io;

use serde::{Deserialize, Serialize};

use serde_with::serde_as;

use zcash_client_backend::wallet::NoteId;

use crate::TryByteArray;
use crate::TxIdWrapper;
use zcash_primitives::transaction::TxId;

use zcash_protocol::ShieldedProtocol;

use super::{ToArray, TryFromArray};

#[derive(Serialize, Deserialize)]
#[serde(remote = "ShieldedProtocol")]
pub enum ShieldedProtocolWrapper {
    Sapling,
    Orchard,
}

impl serde_with::SerializeAs<ShieldedProtocol> for ShieldedProtocolWrapper {
    fn serialize_as<S>(value: &ShieldedProtocol, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: serde::Serializer,
    {
        ShieldedProtocolWrapper::serialize(value, serializer)
    }
}

impl<'de> serde_with::DeserializeAs<'de, ShieldedProtocol> for ShieldedProtocolWrapper {
    fn deserialize_as<D>(deserializer: D) -> Result<ShieldedProtocol, D::Error>
    where
        D: serde::Deserializer<'de>,
    {
        ShieldedProtocolWrapper::deserialize(deserializer).map(Into::into)
    }
}

#[derive(Serialize, Deserialize)]
#[serde(remote = "Scope")]
pub enum ScopeWrapper {
    External,
    Internal,
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

impl ToArray<u8, 43> for sapling::PaymentAddress {
    fn to_array(&self) -> [u8; 43] {
        self.to_bytes()
    }
}

impl TryFromArray<u8, 43> for sapling::PaymentAddress {
    type Error = io::Error;
    fn try_from_array(arr: [u8; 43]) -> Result<Self, Self::Error> {
        sapling::PaymentAddress::from_bytes(&arr)
            .ok_or_else(|| io::Error::new(io::ErrorKind::InvalidData, "Invalid payment address"))
    }
}

#[serde_as]
#[derive(Serialize, Deserialize)]
#[serde(remote = "NoteId")]
pub(crate) struct NoteIdWrapper {
    #[serde(getter = "NoteId::txid")]
    #[serde_as(as = "TxIdWrapper")]
    txid: TxId,
    #[serde(getter = "NoteId::protocol")]
    #[serde_as(as = "ShieldedProtocolWrapper")]
    protocol: ShieldedProtocol,
    #[serde(getter = "NoteId::output_index")]
    output_index: u16,
}

impl From<NoteIdWrapper> for NoteId {
    fn from(def: NoteIdWrapper) -> NoteId {
        NoteId::new(def.txid, def.protocol, def.output_index)
    }
}

impl serde_with::SerializeAs<NoteId> for NoteIdWrapper {
    fn serialize_as<S>(value: &NoteId, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: serde::Serializer,
    {
        NoteIdWrapper::serialize(value, serializer)
    }
}

impl<'de> serde_with::DeserializeAs<'de, NoteId> for NoteIdWrapper {
    fn deserialize_as<D>(deserializer: D) -> Result<NoteId, D::Error>
    where
        D: serde::Deserializer<'de>,
    {
        NoteIdWrapper::deserialize(deserializer).map(Into::into)
    }
}

pub struct RseedWrapper;
#[derive(Serialize, Deserialize)]
enum RseedSerDe {
    BeforeZip212([u8; 32]),
    AfterZip212([u8; 32]),
}
impl serde_with::SerializeAs<Rseed> for RseedWrapper {
    fn serialize_as<S>(value: &Rseed, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: serde::Serializer,
    {
        match value {
            Rseed::BeforeZip212(rcm) => {
                RseedSerDe::BeforeZip212(rcm.to_bytes()).serialize(serializer)
            }
            Rseed::AfterZip212(rseed) => RseedSerDe::AfterZip212(*rseed).serialize(serializer),
        }
    }
}

impl<'de> serde_with::DeserializeAs<'de, Rseed> for RseedWrapper {
    fn deserialize_as<D>(deserializer: D) -> Result<Rseed, D::Error>
    where
        D: serde::Deserializer<'de>,
    {
        let rseed_de = RseedSerDe::deserialize(deserializer)?;
        match rseed_de {
            RseedSerDe::BeforeZip212(rcm) => jubjub::Fr::from_bytes(&rcm)
                .into_option()
                .ok_or_else(|| serde::de::Error::custom("Invalid Rseed"))
                .map(Rseed::BeforeZip212),
            RseedSerDe::AfterZip212(rseed) => Ok(Rseed::AfterZip212(rseed)),
        }
    }
}

pub struct NoteValueWrapper;
impl serde_with::SerializeAs<sapling::value::NoteValue> for NoteValueWrapper {
    fn serialize_as<S>(value: &sapling::value::NoteValue, serializer: S) -> Result<S::Ok, S::Error>
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

#[serde_as]
#[derive(Serialize, Deserialize)]
#[serde(remote = "sapling::Note")]
pub struct SaplingNoteWrapper {
    /// The recipient of the funds.
    #[serde_as(as = "TryByteArray<43>")]
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
#[cfg(feature = "orchard")]
pub use _orchard::*;
#[cfg(feature = "orchard")]
mod _orchard {
    use super::*;
    impl<'de> serde_with::DeserializeAs<'de, orchard::note::Note> for OrchardNoteWrapper {
        fn deserialize_as<D>(deserializer: D) -> Result<orchard::note::Note, D::Error>
        where
            D: serde::Deserializer<'de>,
        {
            #[serde_as]
            #[derive(Deserialize)]
            struct OrchardNoteDe {
                #[serde_as(as = "TryByteArray<43>")]
                recipient: orchard::Address,
                #[serde_as(as = "NoteValueWrapper")]
                value: orchard::value::NoteValue,
                #[serde_as(as = "RhoWrapper")]
                rho: orchard::note::Rho,
                rseed: [u8; 32],
            }
            let OrchardNoteDe {
                recipient,
                value,
                rho,
                rseed,
            } = OrchardNoteDe::deserialize(deserializer)?;
            Ok(orchard::note::Note::from_parts(
                recipient,
                value,
                rho,
                orchard::note::RandomSeed::from_bytes(rseed, &rho)
                    .into_option()
                    .ok_or_else(|| serde::de::Error::custom("Invalid rseed"))?,
            )
            .into_option()
            .ok_or_else(|| serde::de::Error::custom("Invalid orchard note"))?)
        }
    }
    #[serde_as]
    pub struct OrchardNoteWrapper;

    impl serde_with::SerializeAs<orchard::note::Note> for OrchardNoteWrapper {
        fn serialize_as<S>(value: &orchard::note::Note, serializer: S) -> Result<S::Ok, S::Error>
        where
            S: serde::Serializer,
        {
            #[serde_as]
            #[derive(Serialize)]
            struct OrchardNoteSer<'a> {
                #[serde_as(as = "TryByteArray<43>")]
                recipient: orchard::Address,
                #[serde_as(as = "NoteValueWrapper")]
                value: orchard::value::NoteValue,
                #[serde_as(as = "RhoWrapper")]
                rho: orchard::note::Rho,
                rseed: &'a [u8; 32],
            }
            OrchardNoteSer {
                recipient: value.recipient(),
                value: value.value(),
                rho: value.rho(),
                rseed: value.rseed().as_bytes(),
            }
            .serialize(serializer)
        }
    }

    pub struct RhoWrapper;
    impl serde_with::SerializeAs<orchard::note::Rho> for RhoWrapper {
        fn serialize_as<S>(value: &orchard::note::Rho, serializer: S) -> Result<S::Ok, S::Error>
        where
            S: serde::Serializer,
        {
            value.to_bytes().serialize(serializer)
        }
    }
    impl<'de> serde_with::DeserializeAs<'de, orchard::note::Rho> for RhoWrapper {
        fn deserialize_as<D>(deserializer: D) -> Result<orchard::note::Rho, D::Error>
        where
            D: serde::Deserializer<'de>,
        {
            orchard::note::Rho::from_bytes(&<[u8; 32]>::deserialize(deserializer)?)
                .into_option()
                .ok_or_else(|| serde::de::Error::custom("Invalid rho"))
        }
    }

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

    impl ToArray<u8, 43> for orchard::Address {
        fn to_array(&self) -> [u8; 43] {
            self.to_raw_address_bytes()
        }
    }

    impl TryFromArray<u8, 43> for orchard::Address {
        type Error = io::Error;
        fn try_from_array(arr: [u8; 43]) -> Result<Self, Self::Error> {
            orchard::Address::from_raw_address_bytes(&arr)
                .into_option()
                .ok_or_else(|| {
                    io::Error::new(
                        io::ErrorKind::InvalidData,
                        "Invalid orchard payment address",
                    )
                })
        }
    }
}
