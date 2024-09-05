use serde::Deserializer;
use serde::{ser::SerializeStruct, Deserialize, Serialize};
use serde_with::FromInto;
use serde_with::TryFromInto;
use serde_with::{ser::SerializeAsWrap, serde_as};
use zcash_client_backend::{
    data_api::{AccountPurpose, AccountSource},
    wallet::NoteId,
};
use zcash_primitives::{block::BlockHash, transaction::TxId};
use zcash_protocol::consensus::BlockHeight;
use zcash_protocol::memo::Memo;
use zcash_protocol::{memo::MemoBytes, ShieldedProtocol};
use zip32::fingerprint::SeedFingerprint;

#[serde_as]
#[derive(Serialize, Deserialize)]
#[serde(remote = "AccountSource")]
pub enum AccountSourceWrapper {
    /// An account derived from a known seed.
    Derived {
        #[serde_as(as = "SeedFingerprintWrapper")]
        seed_fingerprint: SeedFingerprint,
        #[serde_as(as = "TryFromInto<u32>")]
        account_index: zip32::AccountId,
    },

    /// An account imported from a viewing key.
    Imported {
        #[serde_as(as = "AccountPurposeWrapper")]
        purpose: AccountPurpose,
    },
}
// Provide a conversion to construct the remote type.
impl From<AccountSourceWrapper> for AccountSource {
    fn from(def: AccountSourceWrapper) -> AccountSource {
        match def {
            AccountSourceWrapper::Derived {
                seed_fingerprint,
                account_index,
            } => AccountSource::Derived {
                seed_fingerprint,
                account_index,
            },
            AccountSourceWrapper::Imported { purpose } => AccountSource::Imported { purpose },
        }
    }
}

impl serde_with::SerializeAs<AccountSource> for AccountSourceWrapper {
    fn serialize_as<S>(value: &AccountSource, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: serde::Serializer,
    {
        AccountSourceWrapper::serialize(value, serializer)
    }
}

impl<'de> serde_with::DeserializeAs<'de, AccountSource> for AccountSourceWrapper {
    fn deserialize_as<D>(deserializer: D) -> Result<AccountSource, D::Error>
    where
        D: serde::Deserializer<'de>,
    {
        AccountSourceWrapper::deserialize(deserializer).map(Into::into)
    }
}

pub(crate) struct TxIdWrapper;

impl serde_with::SerializeAs<TxId> for TxIdWrapper {
    fn serialize_as<S>(value: &TxId, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: serde::Serializer,
    {
        value.as_ref().serialize(serializer)
    }
}

impl<'de> serde_with::DeserializeAs<'de, TxId> for TxIdWrapper {
    fn deserialize_as<D>(deserializer: D) -> Result<TxId, D::Error>
    where
        D: serde::Deserializer<'de>,
    {
        Ok(TxId::from_bytes(<[u8; 32]>::deserialize(deserializer)?))
    }
}

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

pub struct MemoBytesWrapper;
impl serde_with::SerializeAs<MemoBytes> for MemoBytesWrapper {
    fn serialize_as<S>(value: &MemoBytes, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: serde::Serializer,
    {
        value.as_slice().serialize(serializer)
    }
}

impl<'de> serde_with::DeserializeAs<'de, MemoBytes> for MemoBytesWrapper {
    fn deserialize_as<D>(deserializer: D) -> Result<MemoBytes, D::Error>
    where
        D: serde::Deserializer<'de>,
    {
        let b = <Vec<u8>>::deserialize(deserializer)?;
        MemoBytes::from_bytes(&b)
                .map_err(|_| serde::de::Error::custom("Invalid memo bytes"))
    }
}

impl serde_with::SerializeAs<Memo> for MemoBytesWrapper {
    fn serialize_as<S>(value: &Memo, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: serde::Serializer,
    {
        value.encode().as_slice().serialize(serializer)
    }
}

impl<'de> serde_with::DeserializeAs<'de, Memo> for MemoBytesWrapper {
    fn deserialize_as<D>(deserializer: D) -> Result<Memo, D::Error>
    where
        D: serde::Deserializer<'de>,
    {
        let b = <Vec<u8>>::deserialize(deserializer)?;
        Memo::from_bytes(&b).map_err(|_| serde::de::Error::custom("Invalid memo"))
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

pub(crate) struct BlockHashWrapper;
impl serde_with::SerializeAs<BlockHash> for BlockHashWrapper {
    fn serialize_as<S>(value: &BlockHash, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: serde::Serializer,
    {
        value.0.serialize(serializer)
    }
}
impl<'de> serde_with::DeserializeAs<'de, BlockHash> for BlockHashWrapper {
    fn deserialize_as<D>(deserializer: D) -> Result<BlockHash, D::Error>
    where
        D: serde::Deserializer<'de>,
    {
        Ok(BlockHash(<[u8; 32]>::deserialize(deserializer)?))
    }
}

pub(crate) struct AccountPurposeWrapper;
impl serde_with::SerializeAs<AccountPurpose> for AccountPurposeWrapper {
    fn serialize_as<S>(value: &AccountPurpose, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: serde::Serializer,
    {
        match value {
            AccountPurpose::Spending => serializer.serialize_str("Spending"),
            AccountPurpose::ViewOnly => serializer.serialize_str("ViewOnly"),
        }
    }
}
impl<'de> serde_with::DeserializeAs<'de, AccountPurpose> for AccountPurposeWrapper {
    fn deserialize_as<D>(deserializer: D) -> Result<AccountPurpose, D::Error>
    where
        D: serde::Deserializer<'de>,
    {
        let s = String::deserialize(deserializer)?;
        match s.as_str() {
            "Spending" => Ok(AccountPurpose::Spending),
            "ViewOnly" => Ok(AccountPurpose::ViewOnly),
            _ => Err(serde::de::Error::custom("Invalid account purpose")),
        }
    }
}

pub(crate) struct SeedFingerprintWrapper;
impl serde_with::SerializeAs<SeedFingerprint> for SeedFingerprintWrapper {
    fn serialize_as<S>(value: &SeedFingerprint, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: serde::Serializer,
    {
        value.to_bytes().serialize(serializer)
    }
}
impl<'de> serde_with::DeserializeAs<'de, SeedFingerprint> for SeedFingerprintWrapper {
    fn deserialize_as<D>(deserializer: D) -> Result<SeedFingerprint, D::Error>
    where
        D: serde::Deserializer<'de>,
    {
        Ok(SeedFingerprint::from_bytes(<[u8; 32]>::deserialize(
            deserializer,
        )?))
    }
}
#[serde_as]
#[derive(Serialize, Deserialize)]
#[serde(remote = "incrementalmerkletree::Address")]
pub struct TreeAddressWrapper {
    #[serde_as(as = "FromInto<u8>")]
    #[serde(getter = "incrementalmerkletree::Address::level")]
    level: incrementalmerkletree::Level,
    #[serde(getter = "incrementalmerkletree::Address::index")]
    index: u64,
}
impl From<TreeAddressWrapper> for incrementalmerkletree::Address {
    fn from(def: TreeAddressWrapper) -> incrementalmerkletree::Address {
        incrementalmerkletree::Address::from_parts(def.level, def.index)
    }
}
impl serde_with::SerializeAs<incrementalmerkletree::Address> for TreeAddressWrapper {
    fn serialize_as<S>(
        value: &incrementalmerkletree::Address,
        serializer: S,
    ) -> Result<S::Ok, S::Error>
    where
        S: serde::Serializer,
    {
        TreeAddressWrapper::serialize(value, serializer)
    }
}
impl<'de> serde_with::DeserializeAs<'de, incrementalmerkletree::Address> for TreeAddressWrapper {
    fn deserialize_as<D>(deserializer: D) -> Result<incrementalmerkletree::Address, D::Error>
    where
        D: serde::Deserializer<'de>,
    {
        TreeAddressWrapper::deserialize(deserializer)
    }
}

/// --- notes.rs ---
use sapling::{value::NoteValue, PaymentAddress, Rseed};
use serde::de::VariantAccess;
use serde_with::{de::DeserializeAsWrap, DeserializeAs, SerializeAs};
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
    fn serialize_as<S>(value: &sapling::PaymentAddress, serializer: S) -> Result<S::Ok, S::Error>
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
        sapling::PaymentAddress::from_bytes(&arrays::deserialize::<_, u8, 43>(deserializer)?)
                .ok_or_else(|| serde::de::Error::custom("Invalid sapling payment address"))
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
        orchard::Address::from_raw_address_bytes(&arrays::deserialize::<_, u8, 43>(
                deserializer,
            )?)
            .into_option()
            .ok_or_else(|| serde::de::Error::custom("Invalid orchard payment address"))
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
                    .next_element::<DeserializeAsWrap<orchard::Address, PaymentAddressWrapper>>()?
                    .ok_or_else(|| serde::de::Error::invalid_length(0, &"a recipient"))?
                    .into_inner();
                let value = seq
                    .next_element::<DeserializeAsWrap<orchard::value::NoteValue, NoteValueWrapper>>(
                    )?
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
                orchard::note::Note::from_parts(recipient, value, rho, rseed)
                        .into_option()
                        .ok_or_else(|| serde::de::Error::custom("Invalid orchard note"))
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
/// --- account.rs ---
use incrementalmerkletree::{
    frontier::{Frontier, NonEmptyFrontier},
    Position,
};
use serde::Serializer;

use zcash_client_backend::data_api::{chain::ChainState, AccountBirthday};

#[serde_as]
#[derive(Serialize, Deserialize)]
#[serde(remote = "zcash_client_backend::data_api::AccountBirthday")]
pub struct AccountBirthdayWrapper {
    #[serde_as(as = "ChainStateWrapper")]
    #[serde(getter = "zcash_client_backend::data_api::AccountBirthday::prior_chain_state")]
    pub prior_chain_state: ChainState,
    #[serde_as(as = "Option<FromInto<u32>>")]
    #[serde(getter = "zcash_client_backend::data_api::AccountBirthday::recover_until")]
    pub recover_until: Option<BlockHeight>,
}
impl SerializeAs<AccountBirthday> for AccountBirthdayWrapper {
    fn serialize_as<S>(source: &AccountBirthday, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: Serializer,
    {
        AccountBirthdayWrapper::serialize(source, serializer)
    }
}

impl<'de> DeserializeAs<'de, AccountBirthday> for AccountBirthdayWrapper {
    fn deserialize_as<D>(deserializer: D) -> Result<AccountBirthday, D::Error>
    where
        D: serde::Deserializer<'de>,
    {
        AccountBirthdayWrapper::deserialize(deserializer)
    }
}

impl From<AccountBirthdayWrapper> for zcash_client_backend::data_api::AccountBirthday {
    fn from(wrapper: AccountBirthdayWrapper) -> Self {
        Self::from_parts(
            wrapper.prior_chain_state,
            wrapper.recover_until.map(Into::into),
        )
    }
}

#[serde_as]
#[derive(Serialize, Deserialize)]
#[serde(remote = "zcash_client_backend::data_api::chain::ChainState")]
pub struct ChainStateWrapper {
    #[serde_as(as = "FromInto<u32>")]
    #[serde(getter = "zcash_client_backend::data_api::chain::ChainState::block_height")]
    pub block_height: BlockHeight,
    #[serde_as(as = "BlockHashWrapper")]
    #[serde(getter = "zcash_client_backend::data_api::chain::ChainState::block_hash")]
    pub block_hash: BlockHash,
    #[serde_as(as = "TryFromInto<SaplingFrontierWrapper>")]
    #[serde(getter = "zcash_client_backend::data_api::chain::ChainState::final_sapling_tree")]
    pub final_sapling_tree: Frontier<sapling::Node, { sapling::NOTE_COMMITMENT_TREE_DEPTH }>,
    #[cfg(feature = "orchard")]
    #[serde_as(as = "TryFromInto<OrchardFrontierWrapper>")]
    #[serde(getter = "zcash_client_backend::data_api::chain::ChainState::final_orchard_tree")]
    pub final_orchard_tree:
        Frontier<orchard::tree::MerkleHashOrchard, { orchard::NOTE_COMMITMENT_TREE_DEPTH as u8 }>,
}
impl SerializeAs<ChainState> for ChainStateWrapper {
    fn serialize_as<S>(source: &ChainState, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: Serializer,
    {
        ChainStateWrapper::serialize(source, serializer)
    }
}

impl<'de> DeserializeAs<'de, ChainState> for ChainStateWrapper {
    fn deserialize_as<D>(deserializer: D) -> Result<ChainState, D::Error>
    where
        D: serde::Deserializer<'de>,
    {
        ChainStateWrapper::deserialize(deserializer)
    }
}

impl From<ChainStateWrapper> for zcash_client_backend::data_api::chain::ChainState {
    fn from(wrapper: ChainStateWrapper) -> Self {
        Self::new(
            wrapper.block_height,
            wrapper.block_hash,
            wrapper.final_sapling_tree,
            #[cfg(feature = "orchard")]
            wrapper.final_orchard_tree,
        )
    }
}

#[derive(Serialize, Deserialize)]
pub struct SaplingFrontierWrapper {
    pub frontier: Option<NonEmptySaplingFrontierWrapper>,
}

#[cfg(feature = "orchard")]
#[derive(Serialize, Deserialize)]
pub struct OrchardFrontierWrapper {
    pub frontier: Option<NonEmptyOrchardFrontierWrapper>,
}

type NonEmptyFrontierSapling = NonEmptyFrontier<sapling::Node>;
#[cfg(feature = "orchard")]
type NonEmptyFrontierOrchard = NonEmptyFrontier<orchard::tree::MerkleHashOrchard>;

#[serde_as]
#[derive(Serialize, Deserialize)]
pub struct NonEmptySaplingFrontierWrapper {
    #[serde_as(as = "FromInto<u64>")]
    pub position: Position,
    #[serde_as(as = "SaplingNodeWrapper")]
    pub leaf: sapling::Node,
    #[serde_as(as = "Vec<SaplingNodeWrapper>")]
    pub ommers: Vec<sapling::Node>,
}

impl From<Frontier<sapling::Node, { sapling::NOTE_COMMITMENT_TREE_DEPTH }>>
    for SaplingFrontierWrapper
{
    fn from(frontier: Frontier<sapling::Node, { sapling::NOTE_COMMITMENT_TREE_DEPTH }>) -> Self {
        match frontier.take().map(|f| f.into_parts()) {
            Some((position, leaf, ommers)) => SaplingFrontierWrapper {
                frontier: Some(NonEmptySaplingFrontierWrapper {
                    position,
                    leaf,
                    ommers,
                }),
            },
            None => SaplingFrontierWrapper { frontier: None },
        }
    }
}

impl TryInto<Frontier<sapling::Node, { sapling::NOTE_COMMITMENT_TREE_DEPTH }>>
    for SaplingFrontierWrapper
{
    type Error = String;
    fn try_into(
        self,
    ) -> Result<Frontier<sapling::Node, { sapling::NOTE_COMMITMENT_TREE_DEPTH }>, Self::Error> {
        match self.frontier {
            Some(n) => {
                let NonEmptySaplingFrontierWrapper {
                    position,
                    leaf,
                    ommers,
                } = n;
                Frontier::from_parts(position, leaf, ommers).map_err(|e| format!("{:?}", e))
            }
            None => Ok(Frontier::empty()),
        }
    }
}
#[cfg(feature = "orchard")]
impl From<Frontier<orchard::tree::MerkleHashOrchard, { orchard::NOTE_COMMITMENT_TREE_DEPTH as u8 }>>
    for OrchardFrontierWrapper
{
    fn from(
        frontier: Frontier<
            orchard::tree::MerkleHashOrchard,
            { orchard::NOTE_COMMITMENT_TREE_DEPTH as u8 },
        >,
    ) -> Self {
        match frontier.take().map(|f| f.into_parts()) {
            Some((position, leaf, ommers)) => OrchardFrontierWrapper {
                frontier: Some(NonEmptyOrchardFrontierWrapper {
                    position,
                    leaf,
                    ommers,
                }),
            },
            None => OrchardFrontierWrapper { frontier: None },
        }
    }
}
#[cfg(feature = "orchard")]
impl
    TryInto<
        Frontier<orchard::tree::MerkleHashOrchard, { orchard::NOTE_COMMITMENT_TREE_DEPTH as u8 }>,
    > for OrchardFrontierWrapper
{
    type Error = String;
    fn try_into(
        self,
    ) -> Result<
        Frontier<orchard::tree::MerkleHashOrchard, { orchard::NOTE_COMMITMENT_TREE_DEPTH as u8 }>,
        Self::Error,
    > {
        match self.frontier {
            Some(n) => {
                let NonEmptyOrchardFrontierWrapper {
                    position,
                    leaf,
                    ommers,
                } = n;
                Frontier::from_parts(position, leaf, ommers).map_err(|e| format!("{:?}", e))
            }
            None => Ok(Frontier::empty()),
        }
    }
}

pub(crate) struct SaplingNodeWrapper;
impl SerializeAs<sapling::Node> for SaplingNodeWrapper {
    fn serialize_as<S>(source: &sapling::Node, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: Serializer,
    {
        source.to_bytes().serialize(serializer)
    }
}
impl<'de> DeserializeAs<'de, sapling::Node> for SaplingNodeWrapper {
    fn deserialize_as<D>(deserializer: D) -> Result<sapling::Node, D::Error>
    where
        D: serde::Deserializer<'de>,
    {
        let bytes = <[u8; 32]>::deserialize(deserializer)?;
        sapling::Node::from_bytes(bytes)
            .into_option()
            .ok_or_else(|| serde::de::Error::custom("Invalid sapling node "))
    }
}

#[cfg(feature = "orchard")]
#[serde_as]
#[derive(Serialize, Deserialize)]
pub struct NonEmptyOrchardFrontierWrapper {
    #[serde_as(as = "FromInto<u64>")]
    pub position: Position,
    #[serde_as(as = "OrchardNodeWrapper")]
    pub leaf: orchard::tree::MerkleHashOrchard,
    #[serde_as(as = "Vec<OrchardNodeWrapper>")]
    pub ommers: Vec<orchard::tree::MerkleHashOrchard>,
}

#[cfg(feature = "orchard")]
pub(crate) struct OrchardNodeWrapper;
#[cfg(feature = "orchard")]
impl SerializeAs<orchard::tree::MerkleHashOrchard> for OrchardNodeWrapper {
    fn serialize_as<S>(
        source: &orchard::tree::MerkleHashOrchard,
        serializer: S,
    ) -> Result<S::Ok, S::Error>
    where
        S: Serializer,
    {
        source.to_bytes().serialize(serializer)
    }
}
#[cfg(feature = "orchard")]
impl<'de> DeserializeAs<'de, orchard::tree::MerkleHashOrchard> for OrchardNodeWrapper {
    fn deserialize_as<D>(deserializer: D) -> Result<orchard::tree::MerkleHashOrchard, D::Error>
    where
        D: serde::Deserializer<'de>,
    {
        let bytes = <[u8; 32]>::deserialize(deserializer)?;
        orchard::tree::MerkleHashOrchard::from_bytes(&bytes)
            .into_option()
            .ok_or_else(|| serde::de::Error::custom("Invalid orchard node "))
    }
}

/// --- nullifier.rs ---
#[cfg(feature = "orchard")]
pub(crate) struct OrchardNullifierWrapper;
#[cfg(feature = "orchard")]
impl serde_with::SerializeAs<orchard::note::Nullifier> for OrchardNullifierWrapper {
    fn serialize_as<S>(value: &orchard::note::Nullifier, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: serde::Serializer,
    {
        value.to_bytes().serialize(serializer)
    }
}
#[cfg(feature = "orchard")]
impl<'de> serde_with::DeserializeAs<'de, orchard::note::Nullifier> for OrchardNullifierWrapper {
    fn deserialize_as<D>(deserializer: D) -> Result<orchard::note::Nullifier, D::Error>
    where
        D: serde::Deserializer<'de>,
    {
        orchard::note::Nullifier::from_bytes(&<[u8; 32]>::deserialize(deserializer)?)
                .into_option()
                .ok_or_else(|| serde::de::Error::custom("Invalid nullifier"))
    }
}

pub(crate) struct SaplingNullifierWrapper;
impl serde_with::SerializeAs<sapling::Nullifier> for SaplingNullifierWrapper {
    fn serialize_as<S>(value: &sapling::Nullifier, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: serde::Serializer,
    {
        value.0.serialize(serializer)
    }
}
impl<'de> serde_with::DeserializeAs<'de, sapling::Nullifier> for SaplingNullifierWrapper {
    fn deserialize_as<D>(deserializer: D) -> Result<sapling::Nullifier, D::Error>
    where
        D: serde::Deserializer<'de>,
    {
        Ok(sapling::Nullifier(<[u8; 32]>::deserialize(deserializer)?))
    }
}

/// --- scanning.rs ---
#[serde_as]
#[derive(Serialize, Deserialize)]
#[serde(remote = "zcash_client_backend::data_api::scanning::ScanPriority")]
pub enum ScanPriorityWrapper {
    /// Block ranges that are ignored have lowest priority.
    Ignored,
    /// Block ranges that have already been scanned will not be re-scanned.
    Scanned,
    /// Block ranges to be scanned to advance the fully-scanned height.
    Historic,
    /// Block ranges adjacent to heights at which the user opened the wallet.
    OpenAdjacent,
    /// Blocks that must be scanned to complete note commitment tree shards adjacent to found notes.
    FoundNote,
    /// Blocks that must be scanned to complete the latest note commitment tree shard.
    ChainTip,
    /// A previously scanned range that must be verified to check it is still in the
    /// main chain, has highest priority.
    Verify,
}
impl serde_with::SerializeAs<zcash_client_backend::data_api::scanning::ScanPriority>
    for ScanPriorityWrapper
{
    fn serialize_as<S>(
        source: &zcash_client_backend::data_api::scanning::ScanPriority,
        serializer: S,
    ) -> Result<S::Ok, S::Error>
    where
        S: Serializer,
    {
        ScanPriorityWrapper::serialize(source, serializer)
    }
}
impl<'de> serde_with::DeserializeAs<'de, zcash_client_backend::data_api::scanning::ScanPriority>
    for ScanPriorityWrapper
{
    fn deserialize_as<D>(
        deserializer: D,
    ) -> Result<zcash_client_backend::data_api::scanning::ScanPriority, D::Error>
    where
        D: Deserializer<'de>,
    {
        ScanPriorityWrapper::deserialize(deserializer)
    }
}

/// --- transaction.rs ---
#[serde_as]
#[derive(Serialize, Deserialize)]
#[serde(remote = "zcash_client_backend::data_api::TransactionStatus")]
pub enum TransactionStatusWrapper {
    /// The requested transaction ID was not recognized by the node.
    TxidNotRecognized,
    /// The requested transaction ID corresponds to a transaction that is recognized by the node,
    /// but is in the mempool or is otherwise not mined in the main chain (but may have been mined
    /// on a fork that was reorged away).
    NotInMainChain,
    /// The requested transaction ID corresponds to a transaction that has been included in the
    /// block at the provided height.
    Mined(#[serde_as(as = "FromInto<u32>")] zcash_primitives::consensus::BlockHeight),
}
pub mod arrays {
    use std::{convert::TryInto, marker::PhantomData};

    use serde::{
        de::{SeqAccess, Visitor},
        ser::SerializeTuple,
        Deserialize, Deserializer, Serialize, Serializer,
    };
    pub fn serialize<S: Serializer, T: Serialize, const N: usize>(
        data: &[T; N],
        ser: S,
    ) -> Result<S::Ok, S::Error> {
        let mut s = ser.serialize_tuple(N)?;
        for item in data {
            s.serialize_element(item)?;
        }
        s.end()
    }

    struct ArrayVisitor<T, const N: usize>(PhantomData<T>);

    impl<'de, T, const N: usize> Visitor<'de> for ArrayVisitor<T, N>
    where
        T: Deserialize<'de>,
    {
        type Value = [T; N];

        fn expecting(&self, formatter: &mut std::fmt::Formatter) -> std::fmt::Result {
            formatter.write_str(&format!("an array of length {}", N))
        }

        #[inline]
        fn visit_seq<A>(self, mut seq: A) -> Result<Self::Value, A::Error>
        where
            A: SeqAccess<'de>,
        {
            // can be optimized using MaybeUninit
            let mut data = Vec::with_capacity(N);
            for _ in 0..N {
                match (seq.next_element())? {
                    Some(val) => data.push(val),
                    None => return Err(serde::de::Error::invalid_length(N, &self)),
                }
            }
            match data.try_into() {
                Ok(arr) => Ok(arr),
                Err(_) => unreachable!(),
            }
        }
    }
    pub fn deserialize<'de, D, T, const N: usize>(deserializer: D) -> Result<[T; N], D::Error>
    where
        D: Deserializer<'de>,
        T: Deserialize<'de>,
    {
        deserializer.deserialize_tuple(N, ArrayVisitor::<T, N>(PhantomData))
    }
}
