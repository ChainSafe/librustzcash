use sapling::{value::NoteValue, PaymentAddress, Rseed};
use serde::de::VariantAccess;
use serde_with::{de::DeserializeAsWrap, DeserializeAs, SerializeAs};
use std::collections::BTreeSet;
use zcash_client_backend::wallet::Note;
use zip32::Scope;

use std::io;
use std::ops::Deref;
use std::sync::Arc;

use incrementalmerkletree::Hashable;
use serde::ser::{SerializeSeq, SerializeTuple};
use serde::Deserializer;
use serde::{ser::SerializeStruct, Deserialize, Serialize};
use serde_with::FromInto;
use serde_with::TryFromInto;
use serde_with::{ser::SerializeAsWrap, serde_as};
use shardtree::store::memory::MemoryShardStore;
use shardtree::store::{Checkpoint, TreeState};
use shardtree::RetentionFlags;
use shardtree::{store::ShardStore, LocatedPrunableTree, Node as TreeNode, PrunableTree};
use std::fmt::Debug;
use zcash_client_backend::data_api::scanning::ScanPriority;
use zcash_client_backend::{
    data_api::{AccountPurpose, AccountSource},
    wallet::NoteId,
};
use zcash_keys::keys::UnifiedFullViewingKey;

use zcash_primitives::{block::BlockHash, transaction::TxId};
use zcash_protocol::consensus::{BlockHeight, MainNetwork};

use crate::ToFromBytes;
use crate::ToFromBytesWrapper;
use crate::TxIdWrapper;
use zcash_protocol::memo::Memo;
use zcash_protocol::{memo::MemoBytes, ShieldedProtocol};
use zip32::fingerprint::SeedFingerprint;

pub(crate) struct ShieldedProtocolWrapper;
impl serde_with::SerializeAs<ShieldedProtocol> for ShieldedProtocolWrapper {
    fn serialize_as<S>(value: &ShieldedProtocol, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: serde::Serializer,
    {
        match value {
            ShieldedProtocol::Sapling => serializer.serialize_str("Sapling"),
            ShieldedProtocol::Orchard => serializer.serialize_str("Orchard"),
        }
    }
}

impl<'de> serde_with::DeserializeAs<'de, ShieldedProtocol> for ShieldedProtocolWrapper {
    fn deserialize_as<D>(deserializer: D) -> Result<ShieldedProtocol, D::Error>
    where
        D: serde::Deserializer<'de>,
    {
        let s = String::deserialize(deserializer)?;
        match s.as_str() {
            "Sapling" => Ok(ShieldedProtocol::Sapling),
            "Orchard" => Ok(ShieldedProtocol::Orchard),
            _ => Err(serde::de::Error::custom("Invalid shielded protocol")),
        }
    }
}

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

impl ToFromBytes for sapling::PaymentAddress {
    fn to_bytes(&self) -> Vec<u8> {
        self.to_bytes().to_vec()
    }

    fn from_bytes(bytes: &[u8]) -> io::Result<Self> {
        sapling::PaymentAddress::from_bytes(
            bytes
                .try_into()
                .map_err(|e| io::Error::new(io::ErrorKind::InvalidData, format!("{}", e)))?,
        )
        .ok_or_else(|| {
            io::Error::new(
                io::ErrorKind::InvalidData,
                "Invalid sapling payment address",
            )
        })
    }
}
#[cfg(feature = "orchard")]
impl ToFromBytes for orchard::Address {
    fn to_bytes(&self) -> Vec<u8> {
        self.to_raw_address_bytes().to_vec()
    }

    fn from_bytes(bytes: &[u8]) -> io::Result<Self> {
        orchard::Address::from_raw_address_bytes(
            bytes
                .try_into()
                .map_err(|e| io::Error::new(io::ErrorKind::InvalidData, format!("{}", e)))?,
        )
        .into_option()
        .ok_or_else(|| {
            io::Error::new(
                io::ErrorKind::InvalidData,
                "Invalid sapling payment address",
            )
        })
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
            Rseed::BeforeZip212(rcm) => {
                serializer.serialize_newtype_variant("Rseed", 0, "BeforeZip212", &rcm.to_bytes())
            }
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
#[cfg(feature = "orchard")]
impl serde_with::SerializeAs<orchard::value::NoteValue> for NoteValueWrapper {
    fn serialize_as<S>(value: &orchard::value::NoteValue, serializer: S) -> Result<S::Ok, S::Error>
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
        orchard::note::Rho::from_bytes(&<[u8; 32]>::deserialize(deserializer)?)
            .into_option()
            .ok_or_else(|| serde::de::Error::custom("Invalid rho"))
    }
}

#[serde_as]
#[derive(Serialize, Deserialize)]
#[serde(remote = "sapling::Note")]
pub struct SaplingNoteWrapper {
    /// The recipient of the funds.
    #[serde_as(as = "ToFromBytesWrapper<PaymentAddress>")]
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
            &SerializeAsWrap::<_, ToFromBytesWrapper<orchard::Address>>::new(&value.recipient()),
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
                            recipient = Some(map.next_value::<DeserializeAsWrap<
                                orchard::Address,
                                ToFromBytesWrapper<orchard::Address>,
                            >>()?);
                        }
                        "value" => {
                            value =
                                Some(map.next_value::<DeserializeAsWrap<
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
                orchard::note::Note::from_parts(recipient, value, rho, rseed)
                    .into_option()
                    .ok_or_else(|| serde::de::Error::custom("Invalid orchard note"))
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
