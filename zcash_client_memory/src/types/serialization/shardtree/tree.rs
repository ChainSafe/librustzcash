use std::collections::BTreeSet;


use std::ops::Deref;
use std::sync::Arc;

use incrementalmerkletree::{Hashable, Position};
use serde::ser::{SerializeSeq, SerializeTuple};
use serde::{ser::SerializeStruct, Deserialize, Serialize};
use serde::{Deserializer, Serializer};
use serde_with::de::DeserializeAsWrap;
use serde_with::{ser::SerializeAsWrap, serde_as};
use serde_with::{DeserializeAs};
use serde_with::{FromInto, SerializeAs};
use shardtree::store::memory::MemoryShardStore;
use shardtree::store::{Checkpoint, TreeState};
use shardtree::RetentionFlags;
use shardtree::{store::ShardStore, LocatedPrunableTree, Node as TreeNode, PrunableTree};
use std::fmt::Debug;







use crate::ToFromBytesWrapper;




use crate::ToFromBytes;

const SER_V1: u8 = 1;

const NIL_TAG: u8 = 0;
const LEAF_TAG: u8 = 1;
const PARENT_TAG: u8 = 2;

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

#[serde_as]
pub struct MemoryShardStoreWrapper;
impl<
        H: Clone + ToFromBytes,
        C: Ord + Clone + From<u32> + Into<u32>, // Most Cases this will be height
        T: ShardStore<H = H, CheckpointId = C>,
    > serde_with::SerializeAs<T> for MemoryShardStoreWrapper
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
                        Ok(shard)
                    })
                    .collect::<Result<Vec<_>, _>>()?,
            ),
        )?;
        let checkpoint_count = value
            .checkpoint_count()
            .map_err(|_| serde::ser::Error::custom("Failed to get checkpoint count"))?;
        let mut checkpoints: Vec<(C, Checkpoint)> = Vec::with_capacity(checkpoint_count);
        let min_checkpoint: u32 = value
            .max_checkpoint_id()
            .map_err(|_| serde::ser::Error::custom("Failed to get max checkpoint id"))?
            .unwrap()
            .into();
        let max_checkpoint: u32 = value
            .min_checkpoint_id()
            .map_err(|_| serde::ser::Error::custom("Failed to get min checkpoint id"))?
            .unwrap()
            .into();

        for checkpoint_id in min_checkpoint..=max_checkpoint {
            let checkpoint = value
                .get_checkpoint(&checkpoint_id.into())
                .map_err(|_| serde::ser::Error::custom("Failed to get checkpoint"))?
                .ok_or_else(|| serde::ser::Error::custom("Missing checkpoint"))?; // TODO: I think we can skip this and just do a length check at the end
            checkpoints.push((checkpoint_id.into(), checkpoint));
        }
        if checkpoints.len() != checkpoint_count {
            return Err(serde::ser::Error::custom(format!(
                "Expected {} checkpoints but got {}",
                checkpoint_count,
                checkpoints.len()
            )));
        }
        s.serialize_field(
            "checkpoints",
            &SerializeAsWrap::<_, Vec<(FromInto<u32>, CheckpointWrapper)>>::new(&checkpoints),
        )?;
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

impl<'de, H: Clone + ToFromBytes, C: Clone + Ord + From<u32> + Into<u32>>
    serde_with::DeserializeAs<'de, MemoryShardStore<H, C>> for MemoryShardStoreWrapper
{
    fn deserialize_as<D>(deserializer: D) -> Result<MemoryShardStore<H, C>, D::Error>
    where
        D: serde::Deserializer<'de>,
    {
        struct Visitor<H, C>(std::marker::PhantomData<(H, C)>);
        impl<H, C> Visitor<H, C> {
            fn new() -> Self {
                Self(std::marker::PhantomData)
            }
        }
        impl<'de, H, C> serde::de::Visitor<'de> for Visitor<H, C>
        where
            H: Clone + ToFromBytes,
            C: Ord + Clone + From<u32> + Into<u32>, // Most Cases this will be height
        {
            type Value = MemoryShardStore<H, C>;
            fn expecting(&self, formatter: &mut std::fmt::Formatter) -> std::fmt::Result {
                formatter.write_str("a MemoryShardStore")
            }
            fn visit_map<A>(self, mut map: A) -> Result<Self::Value, A::Error>
            where
                A: serde::de::MapAccess<'de>,
            {
                let mut shards = None;
                let mut checkpoints = None;
                let mut cap = None;
                while let Some(key) = map.next_key::<String>()? {
                    match key.as_str() {
                        "shards" => {
                            shards = Some (
                                map.next_value::<DeserializeAsWrap<
                                _,
                                Vec<LocatedPrunableTreeWrapper<_>>>>()?
                             );
                        }
                        "checkpoints" => {
                            checkpoints =
                                Some(map.next_value::<DeserializeAsWrap<
                                    _,
                                    Vec<(FromInto<u32>, CheckpointWrapper)>,
                                >>()?);
                        }
                        "cap" => {
                            cap = Some(
                                map.next_value::<DeserializeAsWrap<_, PrunableTreeWrapper>>()?,
                            );
                        }
                        _ => {
                            return Err(serde::de::Error::custom(format!(
                                "Unexpected key: {}",
                                key
                            )));
                        }
                    }
                }
                let shards = shards
                    .ok_or_else(|| serde::de::Error::missing_field("shards"))?
                    .into_inner();
                let checkpoints: Vec<(C, Checkpoint)> = checkpoints
                    .ok_or_else(|| serde::de::Error::missing_field("checkpoints"))?
                    .into_inner();
                let cap = cap
                    .ok_or_else(|| serde::de::Error::missing_field("cap"))?
                    .into_inner();

                let mut store = MemoryShardStore::empty();
                for shard in shards {
                    store
                        .put_shard(shard)
                        .map_err(|_e| serde::de::Error::custom("Failed to put shard into store"))?;
                }
                store
                    .put_cap(cap)
                    .map_err(|_e| serde::de::Error::custom("Failed to put cap into store"))?;
                for (checkpoint_id, checkpoint) in checkpoints {
                    store
                        .add_checkpoint(checkpoint_id, checkpoint)
                        .map_err(|_e| {
                            serde::de::Error::custom("Failed to add checkpoint to store")
                        })?;
                }
                Ok(store)
            }
        }
        deserializer.deserialize_struct(
            "MemoryShardStore",
            &["shards", "checkpoints", "cap"],
            Visitor::<H, C>::new(),
        )
    }
}

pub struct MemoryShardTreeWrapper;
impl<H, C, const DEPTH: u8, const SHARD_HEIGHT: u8>
    SerializeAs<shardtree::ShardTree<MemoryShardStore<H, C>, DEPTH, SHARD_HEIGHT>>
    for MemoryShardTreeWrapper
where
    H: Clone + Hashable + PartialEq + ToFromBytes,
    C: Ord + Clone + Debug + From<u32> + Into<u32>,
{
    fn serialize_as<S>(
        value: &shardtree::ShardTree<MemoryShardStore<H, C>, DEPTH, SHARD_HEIGHT>,
        serializer: S,
    ) -> Result<S::Ok, S::Error>
    where
        S: serde::Serializer,
    {
        let mut s = serializer.serialize_struct("MemoryShardTree", 2)?;

        s.serialize_field(
            "store",
            &SerializeAsWrap::<_, MemoryShardStoreWrapper>::new(value.store()),
        )?;
        s.serialize_field("max_checkpoints", &value.max_checkpoints())?;
        s.end()
    }
}

impl<'de, H, C, const DEPTH: u8, const SHARD_HEIGHT: u8>
    DeserializeAs<'de, shardtree::ShardTree<MemoryShardStore<H, C>, DEPTH, SHARD_HEIGHT>>
    for MemoryShardTreeWrapper
where
    H: Clone + Hashable + PartialEq + ToFromBytes,
    C: Ord + Clone + Debug + From<u32> + Into<u32>,
{
    fn deserialize_as<D>(
        deserializer: D,
    ) -> Result<shardtree::ShardTree<MemoryShardStore<H, C>, DEPTH, SHARD_HEIGHT>, D::Error>
    where
        D: Deserializer<'de>,
    {
        struct Visitor<H, C, const DEPTH: u8, const SHARD_HEIGHT: u8>(
            std::marker::PhantomData<(H, C)>,
        );
        impl<H, C, const DEPTH: u8, const SHARD_HEIGHT: u8> Visitor<H, C, DEPTH, SHARD_HEIGHT> {
            fn new() -> Self {
                Self(std::marker::PhantomData)
            }
        }
        impl<
                'de,
                H: Clone + Hashable + PartialEq + ToFromBytes,
                C: Ord + Clone + Debug + From<u32> + Into<u32>,
                const DEPTH: u8,
                const SHARD_HEIGHT: u8,
            > serde::de::Visitor<'de> for Visitor<H, C, DEPTH, SHARD_HEIGHT>
        {
            type Value = shardtree::ShardTree<MemoryShardStore<H, C>, DEPTH, SHARD_HEIGHT>;
            fn expecting(&self, formatter: &mut std::fmt::Formatter) -> std::fmt::Result {
                formatter.write_str("a ShardTree")
            }
            fn visit_map<A>(self, mut map: A) -> Result<Self::Value, A::Error>
            where
                A: serde::de::MapAccess<'de>,
            {
                let mut store = None;
                let mut max_checkpoints = None;
                while let Some(key) = map.next_key::<String>()? {
                    match key.as_str() {
                        "store" => {
                            store = Some(
                                map.next_value::<DeserializeAsWrap<_, MemoryShardStoreWrapper>>()?,
                            );
                        }
                        "max_checkpoints" => {
                            max_checkpoints = Some(map.next_value::<usize>()?);
                        }
                        _ => {
                            return Err(serde::de::Error::custom(format!(
                                "Unexpected key: {}",
                                key
                            )));
                        }
                    }
                }
                let store = store
                    .ok_or_else(|| serde::de::Error::missing_field("store"))?
                    .into_inner();
                let max_checkpoints = max_checkpoints
                    .ok_or_else(|| serde::de::Error::missing_field("max_checkpoints"))?;
                Ok(shardtree::ShardTree::new(store, max_checkpoints))
            }
        }
        deserializer.deserialize_struct(
            "MemoryShardTree",
            &["store", "max_checkpoints"],
            Visitor::<H, C, DEPTH, SHARD_HEIGHT>::new(),
        )
    }
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
impl<'de, H: ToFromBytes> serde_with::DeserializeAs<'de, LocatedPrunableTree<H>>
    for LocatedPrunableTreeWrapper<H>
{
    fn deserialize_as<D>(deserializer: D) -> Result<LocatedPrunableTree<H>, D::Error>
    where
        D: serde::Deserializer<'de>,
    {
        LocatedPrunableTreeWrapper::deserialize(deserializer)
    }
}

#[serde_as]
#[derive(Serialize, Deserialize)]
#[serde(remote = "shardtree::store::TreeState")]
pub enum TreeStateWrapper {
    /// Checkpoints of the empty tree.
    Empty,
    /// Checkpoint at a (possibly pruned) leaf state corresponding to the
    /// wrapped leaf position.
    AtPosition(#[serde_as(as = "FromInto<u64>")] Position),
}
impl SerializeAs<TreeState> for TreeStateWrapper {
    fn serialize_as<S>(value: &TreeState, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: serde::Serializer,
    {
        TreeStateWrapper::serialize(value, serializer)
    }
}
impl<'de> DeserializeAs<'de, TreeState> for TreeStateWrapper {
    fn deserialize_as<D>(deserializer: D) -> Result<TreeState, D::Error>
    where
        D: serde::Deserializer<'de>,
    {
        TreeStateWrapper::deserialize(deserializer)
    }
}

#[serde_as]
#[derive(Serialize, Deserialize)]
#[serde(remote = "shardtree::store::Checkpoint")]
pub struct CheckpointWrapper {
    #[serde_as(as = "TreeStateWrapper")]
    #[serde(getter = "shardtree::store::Checkpoint::tree_state")]
    pub tree_state: TreeState,
    #[serde_as(as = "BTreeSet<FromInto<u64>>")]
    #[serde(getter = "Checkpoint::marks_removed")]
    pub marks_removed: BTreeSet<Position>,
}
impl From<CheckpointWrapper> for Checkpoint {
    fn from(def: CheckpointWrapper) -> Checkpoint {
        Checkpoint::from_parts(def.tree_state, def.marks_removed)
    }
}
impl serde_with::SerializeAs<Checkpoint> for CheckpointWrapper {
    fn serialize_as<S>(value: &Checkpoint, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: serde::Serializer,
    {
        CheckpointWrapper::serialize(value, serializer)
    }
}
impl<'de> serde_with::DeserializeAs<'de, Checkpoint> for CheckpointWrapper {
    fn deserialize_as<D>(deserializer: D) -> Result<Checkpoint, D::Error>
    where
        D: serde::Deserializer<'de>,
    {
        CheckpointWrapper::deserialize(deserializer)
    }
}
