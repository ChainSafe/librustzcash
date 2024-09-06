use std::collections::BTreeMap;
use std::convert::Infallible;
use std::fmt::Display;
use std::io;
use std::ops::Deref;
use std::sync::Arc;

use incrementalmerkletree::frontier::{self, FrontierError};
use serde::ser::{SerializeSeq, SerializeTuple};
use serde::Deserializer;
use serde::{ser::SerializeStruct, Deserialize, Serialize};
use serde_with::FromInto;
use serde_with::TryFromInto;
use serde_with::{ser::SerializeAsWrap, serde_as};
use shardtree::store::memory::MemoryShardStore;
use shardtree::store::Checkpoint;
use shardtree::RetentionFlags;
use shardtree::{store::ShardStore, LocatedPrunableTree, Node as TreeNode, PrunableTree};
use zcash_client_backend::data_api::scanning::ScanPriority;
use zcash_client_backend::{
    data_api::{AccountPurpose, AccountSource},
    wallet::NoteId,
};
use zcash_keys::keys::UnifiedFullViewingKey;
use zcash_primitives::merkle_tree::HashSer;
use zcash_primitives::{block::BlockHash, transaction::TxId};
use zcash_protocol::consensus::{BlockHeight, MainNetwork};
use zcash_protocol::memo::Memo;
use zcash_protocol::{memo::MemoBytes, ShieldedProtocol};
use zip32::fingerprint::SeedFingerprint;

const SER_V1: u8 = 1;

const NIL_TAG: u8 = 0;
const LEAF_TAG: u8 = 1;
const PARENT_TAG: u8 = 2;
#[serde_as]
#[derive(Serialize, Deserialize)]
struct Test {
    #[serde_as(as = "MemoryShardStoreWrapper")]
    pub x: MemoryShardStore<sapling::Node, BlockHeight>,
}

pub struct PrunableTreeWrapper;
// This is copied from zcash_client_backend/src/serialization/shardtree.rs
impl<H: ToFromBytes> SerializeAs<PrunableTree<H>> for PrunableTreeWrapper {
    fn serialize_as<S>(value: &PrunableTree<H>, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: Serializer,
    {
        fn serialize_inner<H: ToFromBytes, S>(
            tree: &PrunableTree<H>,
            state: &mut S::SerializeSeq,
        ) -> Result<(), S::Error>
        where
            S: Serializer,
        {
            match tree.deref() {
                TreeNode::Parent { ann, left, right } => {
                    state.serialize_element(&PARENT_TAG)?;
                    state.serialize_element(&SerializeAsWrap::<
                        _,
                        Option<ToFromBytesWrapper<Arc<H>>>,
                    >::new(&ann.as_ref()))?;
                    serialize_inner::<H, S>(left, state)?;
                    serialize_inner::<H, S>(right, state)?;
                    Ok(())
                }
                TreeNode::Leaf { value } => {
                    state.serialize_element(&LEAF_TAG)?;
                    state.serialize_element(&SerializeAsWrap::<_, ToFromBytesWrapper<H>>::new(
                        &value.0,
                    ))?;
                    state.serialize_element(&value.1.bits())?;
                    Ok(())
                }
                TreeNode::Nil => {
                    state.serialize_element(&NIL_TAG)?;
                    Ok(())
                }
            }
        }

        let mut state = serializer.serialize_seq(None)?;
        state.serialize_element(&SER_V1)?;
        serialize_inner::<H, S>(value, &mut state)?;
        state.end()
    }
}
impl<'de, H: ToFromBytes> DeserializeAs<'de, PrunableTree<H>> for PrunableTreeWrapper {
    fn deserialize_as<D>(deserializer: D) -> Result<PrunableTree<H>, D::Error>
    where
        D: Deserializer<'de>,
    {
        struct Visitor<H>(std::marker::PhantomData<H>);
        impl<H> Visitor<H> {
            fn new() -> Self {
                Self(std::marker::PhantomData)
            }
        }
        impl<'de, H: ToFromBytes> serde::de::Visitor<'de> for Visitor<H> {
            type Value = PrunableTree<H>;
            fn expecting(&self, formatter: &mut std::fmt::Formatter) -> std::fmt::Result {
                formatter.write_str("a PrunableTree")
            }
            fn visit_seq<A>(self, mut seq: A) -> Result<Self::Value, A::Error>
            where
                A: serde::de::SeqAccess<'de>,
            {
                let version: u8 = seq
                    .next_element()?
                    .ok_or_else(|| serde::de::Error::invalid_length(0, &self))?;
                if version != SER_V1 {
                    return Err(serde::de::Error::custom("Invalid version"));
                }
                let tree = deserialize_inner::<H, A>(&mut seq)?;
                Ok(tree)
            }
        }
        fn deserialize_inner<'de, H: ToFromBytes, A>(
            seq: &mut A,
        ) -> Result<PrunableTree<H>, A::Error>
        where
            A: serde::de::SeqAccess<'de>,
        {
            // TODO: Is this right? We explicitly serialize the nil tag which isnt technically conventional
            let tag = seq.next_element()?.unwrap_or(NIL_TAG);

            match tag {
                PARENT_TAG => {
                    let ann = seq.next_element::<Option::<
                    DeserializeAsWrap<Arc<H>,ToFromBytesWrapper<Arc<H>>>>>()?
                    .ok_or_else(|| serde::de::Error::custom("Read parent tag but failed to read node"))?
                        .map(|ann| ann.into_inner());
                    let left = deserialize_inner::<H, A>(seq)?;
                    let right = deserialize_inner::<H, A>(seq)?;
                    Ok(PrunableTree::parent(ann, left, right))
                }
                LEAF_TAG => {
                    let value = seq
                        .next_element::<DeserializeAsWrap<H, ToFromBytesWrapper<H>>>()?
                        .ok_or_else(|| {
                            serde::de::Error::custom("Read leaf tag but failed to read value")
                        })?
                        .into_inner();
                    let flags = seq
                        .next_element::<u8>()?
                        .ok_or_else(|| {
                            serde::de::Error::custom(
                                "Read leaf tag but failed to read retention flags",
                            )
                        })
                        .map(RetentionFlags::from_bits)?
                        .ok_or_else(|| serde::de::Error::custom("Invalid retention flags"))?;

                    Ok(PrunableTree::leaf((value, flags)))
                }
                NIL_TAG => Ok(PrunableTree::empty()),
                _ => Err(serde::de::Error::custom("Invalid node tag")),
            }
        }
        deserializer.deserialize_seq(Visitor::<H>::new())
    }
}

#[serde_as]
#[derive(Serialize, Deserialize)]
#[serde(remote = "LocatedPrunableTree")]
pub struct LocatedPrunableTreeWrapper<H: ToFromBytes> {
    #[serde_as(as = "TreeAddressWrapper")]
    #[serde(getter = "LocatedPrunableTree::root_addr")]
    pub root_addr: incrementalmerkletree::Address,
    #[serde_as(as = "PrunableTreeWrapper")]
    #[serde(getter = "LocatedPrunableTree::root")]
    pub root: PrunableTree<H>,
}
impl<H: ToFromBytes> From<LocatedPrunableTreeWrapper<H>> for LocatedPrunableTree<H> {
    fn from(def: LocatedPrunableTreeWrapper<H>) -> LocatedPrunableTree<H> {
        LocatedPrunableTree::from_parts(def.root_addr, def.root)
    }
}
impl<H: ToFromBytes> serde_with::SerializeAs<LocatedPrunableTree<H>>
    for LocatedPrunableTreeWrapper<H>
{
    fn serialize_as<S>(value: &LocatedPrunableTree<H>, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: serde::Serializer,
    {
        LocatedPrunableTreeWrapper::serialize(value, serializer)
    }
}

// pub struct MemoryShardStore<H, C: Ord> {
//     shards: Vec<LocatedPrunableTree<H>>,
//     checkpoints: BTreeMap<C, Checkpoint>,
//     cap: PrunableTree<H>,
// }

pub struct MemoryShardStoreWrapper;
impl<H: Clone + ToFromBytes, C: Ord + Clone, T: ShardStore<H = H, CheckpointId = C>>
    serde_with::SerializeAs<T> for MemoryShardStoreWrapper
{
    fn serialize_as<S>(value: &T, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: serde::Serializer,
    {
        let mut s = serializer.serialize_struct("MemoryShardStore", 3)?;

        s.serialize_field(
            "shards",
            &SerializeAsWrap::<_, Vec<LocatedPrunableTreeWrapper<_>>>::new(
                &value
                    .get_shard_roots()
                    .map_err(serde::ser::Error::custom)?
                    .into_iter()
                    .map(|shard_root| {
                        let shard = value
                            .get_shard(shard_root)
                            .map_err(serde::ser::Error::custom)?
                            .ok_or_else(|| serde::ser::Error::custom("Missing shard"))?;
                        // match *shard {}
                        Ok(shard)
                    })
                    .collect::<Result<Vec<_>, _>>()?,
            ),
        )?;
        // s.serialize_field("checkpoints", value)
        s.serialize_field(
            "cap",
            &SerializeAsWrap::<_, PrunableTreeWrapper>::new(
                &value
                    .get_cap()
                    .map_err(|_| serde::ser::Error::custom("Failed to get cap"))?,
            ),
        )?;
        s.end()
    }
}
impl<'de, H, C: Ord> serde_with::DeserializeAs<'de, MemoryShardStore<H, C>>
    for MemoryShardStoreWrapper
{
    fn deserialize_as<D>(deserializer: D) -> Result<MemoryShardStore<H, C>, D::Error>
    where
        D: serde::Deserializer<'de>,
    {
        todo!()
    }
}

pub struct UnifiedFullViewingKeyWrapper;
impl serde_with::SerializeAs<UnifiedFullViewingKey> for UnifiedFullViewingKeyWrapper {
    fn serialize_as<S>(value: &UnifiedFullViewingKey, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: serde::Serializer,
    {
        value.encode(&MainNetwork).serialize(serializer)
    }
}
impl<'de> serde_with::DeserializeAs<'de, UnifiedFullViewingKey> for UnifiedFullViewingKeyWrapper {
    fn deserialize_as<D>(deserializer: D) -> Result<UnifiedFullViewingKey, D::Error>
    where
        D: serde::Deserializer<'de>,
    {
        let b = String::deserialize(deserializer)?;
        UnifiedFullViewingKey::decode(&MainNetwork, &b)
            .map_err(|_| serde::de::Error::custom("Invalid unified full viewing key"))
    }
}

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
        MemoBytes::from_bytes(&b).map_err(|_| serde::de::Error::custom("Invalid memo bytes"))
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

// pub struct PaymentAddressWrapper;

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
type Ll = ToFromBytesWrapper<sapling::Node>;

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
    #[serde_as(as = "FrontierWrapper<sapling::Node>")]
    #[serde(getter = "zcash_client_backend::data_api::chain::ChainState::final_sapling_tree")]
    pub final_sapling_tree: Frontier<sapling::Node, { sapling::NOTE_COMMITMENT_TREE_DEPTH }>,
    #[cfg(feature = "orchard")]
    #[serde_as(as = "FrontierWrapper<orchard::tree::MerkleHashOrchard>")]
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

pub struct FrontierWrapper<T: ToFromBytes + Clone> {
    pub frontier: Option<NonEmptyFrontier<T>>,
}
impl<T: ToFromBytes + Clone, const DEPTH: u8> SerializeAs<Frontier<T, DEPTH>>
    for FrontierWrapper<T>
{
    fn serialize_as<S>(value: &Frontier<T, DEPTH>, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: serde::Serializer,
    {
        let mut s = serializer.serialize_struct("Frontier", 1)?;
        s.serialize_field(
            "frontier",
            &SerializeAsWrap::<_, Option<NonEmptyFrontierWrapper<T>>>::new(&value.value().cloned()),
        )?;
        s.end()
    }
}
impl<'de, T: ToFromBytes + Clone, const DEPTH: u8> DeserializeAs<'de, Frontier<T, DEPTH>>
    for FrontierWrapper<T>
{
    fn deserialize_as<D>(deserializer: D) -> Result<Frontier<T, DEPTH>, D::Error>
    where
        D: serde::Deserializer<'de>,
    {
        struct Visitor<T, const DEPTH: u8>(std::marker::PhantomData<T>);
        impl<T, const DEPTH: u8> Visitor<T, DEPTH> {
            fn new() -> Self {
                Self(std::marker::PhantomData)
            }
        }
        impl<'de, T: ToFromBytes + Clone, const DEPTH: u8> serde::de::Visitor<'de> for Visitor<T, DEPTH> {
            type Value = Frontier<T, DEPTH>;
            fn expecting(&self, formatter: &mut std::fmt::Formatter) -> std::fmt::Result {
                formatter.write_str("struct Frontier")
            }
            fn visit_map<A>(self, mut map: A) -> Result<Frontier<T, DEPTH>, A::Error>
            where
                A: serde::de::MapAccess<'de>,
            {
                let mut frontier = None;
                while let Some(key) = map.next_key()? {
                    match key {
                        "frontier" => {
                            frontier = map
                                .next_value::<Option<
                                    DeserializeAsWrap<
                                        NonEmptyFrontier<T>,
                                        NonEmptyFrontierWrapper<T>,
                                    >,
                                >>()?
                                .map(|f| f.into_inner());
                        }
                        _ => {
                            return Err(serde::de::Error::unknown_field(key, &["frontier"]));
                        }
                    }
                }
                frontier
                    .map(NonEmptyFrontier::into_parts)
                    .map(|(p, l, o)| {
                        frontier::Frontier::from_parts(p, l, o).map_err(|_e| {
                            serde::de::Error::custom("failed to construct frontier from parts")
                        })
                    })
                    .transpose()?
                    .ok_or_else(|| serde::de::Error::missing_field("frontier"))
            }
        }
        deserializer.deserialize_struct("Frontier", &["frontier"], Visitor::<T, DEPTH>::new())
    }
}

pub struct NonEmptyFrontierWrapper<T: ToFromBytes> {
    pub position: Position,
    pub leaf: T,
    pub ommers: Vec<T>,
}

impl<T: ToFromBytes> SerializeAs<NonEmptyFrontier<T>> for NonEmptyFrontierWrapper<T> {
    fn serialize_as<S>(value: &NonEmptyFrontier<T>, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: serde::Serializer,
    {
        let ommers = value
            .ommers()
            .iter()
            .map(|o| SerializeAsWrap::<_, ToFromBytesWrapper<T>>::new(o))
            .collect::<Vec<_>>();
        let mut s = serializer.serialize_struct("NonEmptyFrontier", 3)?;
        s.serialize_field(
            "position",
            &SerializeAsWrap::<_, FromInto<u64>>::new(&value.position()),
        )?;
        s.serialize_field(
            "leaf",
            &SerializeAsWrap::<_, ToFromBytesWrapper<T>>::new(&value.leaf()),
        )?;
        s.serialize_field("ommers", &ommers)?;
        s.end()
    }
}

impl<'de, T: ToFromBytes> DeserializeAs<'de, NonEmptyFrontier<T>> for NonEmptyFrontierWrapper<T> {
    fn deserialize_as<D>(deserializer: D) -> Result<NonEmptyFrontier<T>, D::Error>
    where
        D: serde::Deserializer<'de>,
    {
        struct Visitor<T>(std::marker::PhantomData<T>);
        impl<T> Visitor<T> {
            fn new() -> Self {
                Self(std::marker::PhantomData)
            }
        }
        impl<'de, T: ToFromBytes> serde::de::Visitor<'de> for Visitor<T> {
            type Value = NonEmptyFrontier<T>;
            fn expecting(&self, formatter: &mut std::fmt::Formatter) -> std::fmt::Result {
                formatter.write_str("struct OrchardNote")
            }
            fn visit_map<A>(self, mut map: A) -> Result<Self::Value, A::Error>
            where
                A: serde::de::MapAccess<'de>,
            {
                let mut position = None;
                let mut leaf = None;
                let mut ommers = None;
                while let Some(key) = map.next_key()? {
                    match key {
                        "position" => {
                            position = Some(
                                map.next_value::<DeserializeAsWrap<Position, FromInto<u64>>>()?,
                            );
                        }
                        "leaf" => {
                            leaf = Some(
                                map.next_value::<DeserializeAsWrap<T, ToFromBytesWrapper<T>>>()?,
                            );
                        }
                        "ommers" => {
                            ommers = Some(
                                map.next_value::<Vec<DeserializeAsWrap<T, ToFromBytesWrapper<T>>>>(
                                )?,
                            );
                        }
                        _ => {
                            return Err(serde::de::Error::unknown_field(
                                key,
                                &["recipient", "value", "rho", "rseed"],
                            ));
                        }
                    }
                }
                let position = position
                    .ok_or_else(|| serde::de::Error::missing_field("position"))?
                    .into_inner();
                let leaf = leaf
                    .ok_or_else(|| serde::de::Error::missing_field("leaf"))?
                    .into_inner();
                let ommers = ommers
                    .ok_or_else(|| serde::de::Error::missing_field("ommers"))?
                    .into_iter()
                    .map(|o| o.into_inner())
                    .collect();

                NonEmptyFrontier::from_parts(position, leaf, ommers).map_err(|_e| {
                    serde::de::Error::custom("Failed to deserialize non-empty frontier")
                })
            }
        }
        deserializer.deserialize_struct(
            "NonEmptyFrontier",
            &["position", "leaf", "ommers"],
            Visitor::<T>::new(),
        )
    }
}

pub trait ToFromBytes {
    /// Serializes this node into a byte vector.
    fn to_bytes(&self) -> Vec<u8>;

    /// Parses a node from a byte vector.
    fn from_bytes(bytes: &[u8]) -> io::Result<Self>
    where
        Self: Sized;
}

impl<T: ToFromBytes> ToFromBytes for Arc<T> {
    fn to_bytes(&self) -> Vec<u8> {
        self.as_ref().to_bytes()
    }

    fn from_bytes(bytes: &[u8]) -> io::Result<Self> {
        T::from_bytes(bytes).map(Arc::new)
    }
}

#[serde_as]
pub struct ToFromBytesWrapper<T: ToFromBytes>(T);

impl<T: ToFromBytes> SerializeAs<T> for ToFromBytesWrapper<T> {
    fn serialize_as<S>(value: &T, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: serde::Serializer,
    {
        value.to_bytes().serialize(serializer)
    }
}
impl<T: ToFromBytes> SerializeAs<&T> for ToFromBytesWrapper<T> {
    fn serialize_as<S>(value: &&T, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: serde::Serializer,
    {
        value.to_bytes().serialize(serializer)
    }
}
impl<'de, T: ToFromBytes> DeserializeAs<'de, T> for ToFromBytesWrapper<T> {
    fn deserialize_as<D>(deserializer: D) -> Result<T, D::Error>
    where
        D: serde::Deserializer<'de>,
    {
        T::from_bytes(<Vec<u8>>::deserialize(deserializer)?.as_slice())
            .map_err(|e| serde::de::Error::custom(e))
    }
}
impl<T: ToFromBytes> Serialize for ToFromBytesWrapper<T> {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: serde::Serializer,
    {
        ToFromBytesWrapper::<T>::serialize_as(&self.0, serializer)
    }
}
impl<'de, T: ToFromBytes> Deserialize<'de> for ToFromBytesWrapper<T> {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: serde::Deserializer<'de>,
    {
        ToFromBytesWrapper::<T>::deserialize_as(deserializer).map(ToFromBytesWrapper)
    }
}

impl ToFromBytes for sapling::Node {
    fn to_bytes(&self) -> Vec<u8> {
        self.to_bytes().to_vec()
    }

    fn from_bytes(bytes: &[u8]) -> io::Result<Self> {
        let repr: [u8; 32] = bytes.try_into().map_err(|_| {
            io::Error::new(
                io::ErrorKind::InvalidData,
                "Invalid length for Jubjub base field value.",
            )
        })?;
        Option::from(Self::from_bytes(repr)).ok_or_else(|| {
            io::Error::new(
                io::ErrorKind::InvalidData,
                "Non-canonical encoding of Jubjub base field value.",
            )
        })
    }
}

#[cfg(feature = "orchard")]
impl ToFromBytes for orchard::tree::MerkleHashOrchard {
    fn to_bytes(&self) -> Vec<u8> {
        self.to_bytes().to_vec()
    }

    fn from_bytes(bytes: &[u8]) -> io::Result<Self> {
        let repr: [u8; 32] = bytes.try_into().map_err(|_| {
            io::Error::new(
                io::ErrorKind::InvalidData,
                "Invalid length for Pallas base field value.",
            )
        })?;
        <Option<_>>::from(Self::from_bytes(&repr)).ok_or_else(|| {
            io::Error::new(
                io::ErrorKind::InvalidData,
                "Non-canonical encoding of Pallas base field value.",
            )
        })
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
impl SerializeAs<ScanPriority> for ScanPriorityWrapper {
    fn serialize_as<S>(value: &ScanPriority, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: serde::Serializer,
    {
        ScanPriorityWrapper::serialize(value, serializer)
    }
}
impl<'de> DeserializeAs<'de, ScanPriority> for ScanPriorityWrapper {
    fn deserialize_as<D>(deserializer: D) -> Result<ScanPriority, D::Error>
    where
        D: serde::Deserializer<'de>,
    {
        ScanPriorityWrapper::deserialize(deserializer).map(Into::into)
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
