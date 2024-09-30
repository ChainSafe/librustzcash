use sapling::{value::NoteValue, PaymentAddress, Rseed};
use serde_with::{DeserializeAs, SerializeAs};

use zcash_address::ZcashAddress;
use zcash_client_backend::wallet::Note;
use zip32::Scope;

use std::io;

use serde::{Deserialize, Deserializer, Serialize, Serializer};

use serde_with::serde_as;

use zcash_client_backend::wallet::{NoteId, Recipient};

use crate::{ByteArray, TransparentAddressDef, TryByteArray, ZcashAddressDef};

use zcash_primitives::{
    legacy::TransparentAddress,
    transaction::{components::OutPoint, TxId},
};

use zcash_protocol::{PoolType, ShieldedProtocol};

use super::{ToArray, TryFromArray};


#[serde_as]
#[derive(Serialize, Deserialize)]
/// A wrapper for serializing and deserializing arrays as fixed byte sequences that can fail.
pub struct TryByteArray<const N: usize>(#[serde_as(as = "Bytes")] pub [u8; N]);
impl<T: TryToArray<u8, N>, const N: usize> SerializeAs<T> for TryByteArray<N> {
    fn serialize_as<S>(value: &T, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: serde::Serializer,
    {
        ByteArray(value.try_to_array().map_err(serde::ser::Error::custom)?)
            .serialize(serializer)
    }
}
impl<'de, T: TryFromArray<u8, N>, const N: usize> DeserializeAs<'de, T> for TryByteArray<N> {
    fn deserialize_as<D>(deserializer: D) -> Result<T, D::Error>
    where
        D: Deserializer<'de>,
    {
        T::try_from_array(ByteArray::<N>::deserialize(deserializer)?.0)
            .map_err(serde::de::Error::custom)
    }
}


#[serde_as]
#[derive(Serialize, Deserialize)]
#[serde(remote = "Note")]
pub enum NoteDef {
    Sapling(#[serde_as(as = "SaplingNoteDef")] sapling::Note),
    #[cfg(feature = "orchard")]
    Orchard(#[serde_as(as = "OrchardNoteDef")] orchard::Note),
}

#[serde_as]
#[derive(Serialize, Deserialize)]
#[serde(remote = "sapling::Note")]
pub struct SaplingNoteDef {
    /// The recipient of the funds.
    #[serde_as(as = "TryByteArray<43>")]
    #[serde(getter = "sapling::Note::recipient")]
    recipient: PaymentAddress,
    /// The value of this note.
    #[serde_as(as = "NoteValueDef")]
    #[serde(getter = "sapling::Note::value")]
    value: NoteValue,
    /// The seed randomness for various note components.
    #[serde(getter = "sapling::Note::rseed")]
    #[serde_as(as = "RseedDef")]
    rseed: Rseed,
}

#[serde_as]
#[derive(Serialize, Deserialize)]
#[serde(remote = "NoteId")]
pub(crate) struct NoteIdDef {
    #[serde(getter = "NoteId::txid")]
    #[serde_as(as = "ByteArray<32>")]
    txid: TxId,
    #[serde(getter = "NoteId::protocol")]
    #[serde_as(as = "ShieldedProtocolDef")]
    protocol: ShieldedProtocol,
    #[serde(getter = "NoteId::output_index")]
    output_index: u16,
}

#[derive(Serialize, Deserialize)]
#[serde(remote = "ShieldedProtocol")]
pub enum ShieldedProtocolDef {
    Sapling,
    Orchard,
}

#[derive(Serialize, Deserialize)]
#[serde(remote = "Scope")]
pub enum ScopeDef {
    External,
    Internal,
}

pub struct NoteValueDef;
impl SerializeAs<sapling::value::NoteValue> for NoteValueDef {
    fn serialize_as<S>(value: &sapling::value::NoteValue, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: Serializer,
    {
        value.inner().serialize(serializer)
    }
}
impl<'de> serde_with::DeserializeAs<'de, sapling::value::NoteValue> for NoteValueDef {
    fn deserialize_as<D>(deserializer: D) -> Result<sapling::value::NoteValue, D::Error>
    where
        D: Deserializer<'de>,
    {
        Ok(sapling::value::NoteValue::from_raw(u64::deserialize(
            deserializer,
        )?))
    }
}

pub struct RseedDef;
#[derive(Serialize, Deserialize)]
enum RseedSerDe {
    BeforeZip212([u8; 32]),
    AfterZip212([u8; 32]),
}
impl SerializeAs<Rseed> for RseedDef {
    fn serialize_as<S>(value: &Rseed, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: Serializer,
    {
        match value {
            Rseed::BeforeZip212(rcm) => {
                RseedSerDe::BeforeZip212(rcm.to_bytes()).serialize(serializer)
            }
            Rseed::AfterZip212(rseed) => RseedSerDe::AfterZip212(*rseed).serialize(serializer),
        }
    }
}

impl<'de> serde_with::DeserializeAs<'de, Rseed> for RseedDef {
    fn deserialize_as<D>(deserializer: D) -> Result<Rseed, D::Error>
    where
        D: Deserializer<'de>,
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

#[serde_as]
#[derive(Serialize, Deserialize)]
#[serde(remote = "PoolType")]
pub enum PoolTypeDef {
    /// The transparent value pool
    Transparent,
    /// A shielded value pool.
    Shielded(#[serde_as(as = "ShieldedProtocolDef")] ShieldedProtocol),
}

#[serde_as]
#[derive(Serialize, Deserialize)]
#[serde(remote = "OutPoint")]
pub struct OutPointDef {
    #[serde_as(as = "ByteArray<32>")]
    #[serde(getter = "OutPoint::txid")]
    pub hash: TxId,
    #[serde(getter = "OutPoint::n")]
    pub n: u32,
}

pub struct RecipientDef<AccountId, Note, OutPoint>(
    std::marker::PhantomData<(AccountId, Note, OutPoint)>,
);

impl SerializeAs<Recipient<crate::AccountId, Note, OutPoint>>
    for RecipientDef<crate::AccountId, Note, OutPoint>
{
    fn serialize_as<S>(
        value: &Recipient<crate::AccountId, Note, OutPoint>,
        serializer: S,
    ) -> Result<S::Ok, S::Error>
    where
        S: serde::Serializer,
    {
        #[serde_as]
        #[derive(Serialize)]
        pub enum RecipientSer<'a> {
            External(
                #[serde_as(as = "ZcashAddressDef")] &'a ZcashAddress,
                #[serde_as(as = "PoolTypeDef")] &'a PoolType,
            ),
            EphemeralTransparent {
                receiving_account: &'a crate::AccountId,
                #[serde_as(as = "TransparentAddressDef")]
                ephemeral_address: &'a TransparentAddress,
                #[serde_as(as = "OutPointDef")]
                outpoint_metadata: &'a OutPoint,
            },
            InternalAccount {
                receiving_account: &'a crate::AccountId,
                #[serde_as(as = "Option<ZcashAddressDef>")]
                external_address: &'a Option<ZcashAddress>,

                #[serde_as(as = "NoteDef")]
                note: &'a Note,
            },
        }
        let v = match value {
            Recipient::External(address, pool) => RecipientSer::External(address, pool),
            Recipient::EphemeralTransparent {
                receiving_account,
                ephemeral_address,
                outpoint_metadata,
            } => RecipientSer::EphemeralTransparent {
                receiving_account,
                ephemeral_address,
                outpoint_metadata,
            },
            Recipient::InternalAccount {
                receiving_account,
                external_address,
                note,
            } => RecipientSer::InternalAccount {
                receiving_account,
                external_address,
                note,
            },
        };
        RecipientSer::serialize(&v, serializer)
    }
}

impl<'de> DeserializeAs<'de, Recipient<crate::AccountId, Note, OutPoint>>
    for RecipientDef<crate::AccountId, Note, OutPoint>
{
    fn deserialize_as<D>(
        deserializer: D,
    ) -> Result<Recipient<crate::AccountId, Note, OutPoint>, D::Error>
    where
        D: Deserializer<'de>,
    {
        #[serde_as]
        #[derive(Deserialize)]
        pub enum RecipientDe {
            External(
                #[serde_as(as = "ZcashAddressDef")] ZcashAddress,
                #[serde_as(as = "PoolTypeDef")] PoolType,
            ),
            EphemeralTransparent {
                receiving_account: crate::AccountId,
                #[serde_as(as = "TransparentAddressDef")]
                ephemeral_address: TransparentAddress,
                #[serde_as(as = "OutPointDef")]
                outpoint_metadata: OutPoint,
            },
            InternalAccount {
                receiving_account: crate::AccountId,
                #[serde_as(as = "Option<ZcashAddressDef>")]
                external_address: Option<ZcashAddress>,
                #[serde_as(as = "NoteDef")]
                note: Note,
            },
        }
        let v = RecipientDe::deserialize(deserializer)?;
        match v {
            RecipientDe::External(address, pool) => Ok(Recipient::External(address, pool)),
            RecipientDe::EphemeralTransparent {
                receiving_account,
                ephemeral_address,
                outpoint_metadata,
            } => Ok(Recipient::EphemeralTransparent {
                receiving_account,
                ephemeral_address,
                outpoint_metadata,
            }),
            RecipientDe::InternalAccount {
                receiving_account,
                external_address,
                note,
            } => Ok(Recipient::InternalAccount {
                receiving_account,
                external_address,
                note,
            }),
        }
    }
}

// BOILERPLATE: Trivial conversions between types and the trivial implementations of SerializeAs and DeserializeAs

impl From<OutPointDef> for OutPoint {
    fn from(def: OutPointDef) -> OutPoint {
        OutPoint::new(def.hash.into(), def.n)
    }
}

impl SerializeAs<OutPoint> for OutPointDef {
    fn serialize_as<S>(value: &OutPoint, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: Serializer,
    {
        OutPointDef::serialize(value, serializer)
    }
}

impl<'de> DeserializeAs<'de, OutPoint> for OutPointDef {
    fn deserialize_as<D>(deserializer: D) -> Result<OutPoint, D::Error>
    where
        D: Deserializer<'de>,
    {
        OutPointDef::deserialize(deserializer).map(Into::into)
    }
}

impl SerializeAs<PoolType> for PoolTypeDef {
    fn serialize_as<S>(value: &PoolType, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: Serializer,
    {
        PoolTypeDef::serialize(value, serializer)
    }
}

impl<'de> DeserializeAs<'de, PoolType> for PoolTypeDef {
    fn deserialize_as<D>(deserializer: D) -> Result<PoolType, D::Error>
    where
        D: Deserializer<'de>,
    {
        PoolTypeDef::deserialize(deserializer)
    }
}

impl SerializeAs<ShieldedProtocol> for ShieldedProtocolDef {
    fn serialize_as<S>(value: &ShieldedProtocol, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: Serializer,
    {
        ShieldedProtocolDef::serialize(value, serializer)
    }
}

impl<'de> serde_with::DeserializeAs<'de, ShieldedProtocol> for ShieldedProtocolDef {
    fn deserialize_as<D>(deserializer: D) -> Result<ShieldedProtocol, D::Error>
    where
        D: Deserializer<'de>,
    {
        ShieldedProtocolDef::deserialize(deserializer).map(Into::into)
    }
}

impl SerializeAs<Scope> for ScopeDef {
    fn serialize_as<S>(value: &Scope, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: Serializer,
    {
        ScopeDef::serialize(value, serializer)
    }
}
impl<'de> serde_with::DeserializeAs<'de, Scope> for ScopeDef {
    fn deserialize_as<D>(deserializer: D) -> Result<Scope, D::Error>
    where
        D: Deserializer<'de>,
    {
        ScopeDef::deserialize(deserializer).map(Into::into)
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
impl From<NoteIdDef> for NoteId {
    fn from(def: NoteIdDef) -> NoteId {
        NoteId::new(def.txid, def.protocol, def.output_index)
    }
}

impl SerializeAs<NoteId> for NoteIdDef {
    fn serialize_as<S>(value: &NoteId, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: Serializer,
    {
        NoteIdDef::serialize(value, serializer)
    }
}

impl<'de> serde_with::DeserializeAs<'de, NoteId> for NoteIdDef {
    fn deserialize_as<D>(deserializer: D) -> Result<NoteId, D::Error>
    where
        D: Deserializer<'de>,
    {
        NoteIdDef::deserialize(deserializer).map(Into::into)
    }
}

impl From<SaplingNoteDef> for sapling::Note {
    fn from(note: SaplingNoteDef) -> Self {
        sapling::Note::from_parts(note.recipient, note.value, note.rseed)
    }
}
impl SerializeAs<sapling::Note> for SaplingNoteDef {
    fn serialize_as<S>(value: &sapling::Note, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: Serializer,
    {
        SaplingNoteDef::serialize(value, serializer)
    }
}
impl<'de> serde_with::DeserializeAs<'de, sapling::Note> for SaplingNoteDef {
    fn deserialize_as<D>(deserializer: D) -> Result<sapling::Note, D::Error>
    where
        D: Deserializer<'de>,
    {
        SaplingNoteDef::deserialize(deserializer).map(Into::into)
    }
}

impl From<NoteDef> for Note {
    fn from(note: NoteDef) -> Self {
        match note {
            NoteDef::Sapling(inner) => Note::Sapling(inner),
            #[cfg(feature = "orchard")]
            NoteDef::Orchard(inner) => Note::Orchard(inner),
        }
    }
}

impl SerializeAs<Note> for NoteDef {
    fn serialize_as<S>(value: &Note, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: Serializer,
    {
        NoteDef::serialize(value, serializer)
    }
}
impl<'de> DeserializeAs<'de, Note> for NoteDef {
    fn deserialize_as<D>(deserializer: D) -> Result<Note, D::Error>
    where
        D: Deserializer<'de>,
    {
        NoteDef::deserialize(deserializer).map(Into::into)
    }
}

#[cfg(feature = "orchard")]
pub use _orchard::*;
#[cfg(feature = "orchard")]
mod _orchard {
    use crate::TryByteArray;

    use super::*;

    pub struct OrchardNoteDef;
    impl SerializeAs<orchard::note::Note> for OrchardNoteDef {
        fn serialize_as<S>(value: &orchard::note::Note, serializer: S) -> Result<S::Ok, S::Error>
        where
            S: Serializer,
        {
            #[serde_as]
            #[derive(Serialize)]
            struct OrchardNoteSer<'a> {
                #[serde_as(as = "TryByteArray<43>")]
                recipient: orchard::Address,
                #[serde_as(as = "NoteValueDef")]
                value: orchard::value::NoteValue,
                #[serde_as(as = "TryByteArray<32>")]
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

    impl<'de> serde_with::DeserializeAs<'de, orchard::note::Note> for OrchardNoteDef {
        fn deserialize_as<D>(deserializer: D) -> Result<orchard::note::Note, D::Error>
        where
            D: Deserializer<'de>,
        {
            #[serde_as]
            #[derive(Deserialize)]
            struct OrchardNoteDe {
                #[serde_as(as = "TryByteArray<43>")]
                recipient: orchard::Address,
                #[serde_as(as = "NoteValueDef")]
                value: orchard::value::NoteValue,
                #[serde_as(as = "TryByteArray<32>")]
                rho: orchard::note::Rho,
                rseed: [u8; 32],
            }
            let OrchardNoteDe {
                recipient,
                value,
                rho,
                rseed,
            } = OrchardNoteDe::deserialize(deserializer)?;
            orchard::note::Note::from_parts(
                recipient,
                value,
                rho,
                orchard::note::RandomSeed::from_bytes(rseed, &rho)
                    .into_option()
                    .ok_or_else(|| serde::de::Error::custom("Invalid rseed"))?,
            )
            .into_option()
            .ok_or_else(|| serde::de::Error::custom("Invalid orchard note"))
        }
    }

    impl SerializeAs<orchard::value::NoteValue> for NoteValueDef {
        fn serialize_as<S>(
            value: &orchard::value::NoteValue,
            serializer: S,
        ) -> Result<S::Ok, S::Error>
        where
            S: Serializer,
        {
            value.inner().serialize(serializer)
        }
    }
    impl<'de> serde_with::DeserializeAs<'de, orchard::value::NoteValue> for NoteValueDef {
        fn deserialize_as<D>(deserializer: D) -> Result<orchard::value::NoteValue, D::Error>
        where
            D: Deserializer<'de>,
        {
            Ok(orchard::value::NoteValue::from_raw(u64::deserialize(
                deserializer,
            )?))
        }
    }

    impl ToArray<u8, 32> for orchard::note::Rho {
        fn to_array(&self) -> [u8; 32] {
            self.to_bytes()
        }
    }
    impl TryFromArray<u8, 32> for orchard::note::Rho {
        type Error = io::Error;
        fn try_from_array(arr: [u8; 32]) -> Result<Self, Self::Error> {
            orchard::note::Rho::from_bytes(&arr)
                .into_option()
                .ok_or_else(|| io::Error::new(io::ErrorKind::InvalidData, "Invalid rho"))
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
