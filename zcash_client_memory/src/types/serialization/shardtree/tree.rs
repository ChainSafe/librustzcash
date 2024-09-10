use std::collections::{BTreeMap, BTreeSet};

use std::ops::Deref;
use std::sync::Arc;

use incrementalmerkletree::{Address, Level, Position};
use serde::ser::SerializeSeq;
use serde::{Deserialize, Serialize};
use serde::{Deserializer, Serializer};
use serde_with::serde_as;
use serde_with::DeserializeAs;
use serde_with::{FromInto, SerializeAs};
use shardtree::store::memory::MemoryShardStore;
use shardtree::store::{Checkpoint, TreeState};
use shardtree::{store::ShardStore, LocatedPrunableTree, Node, PrunableTree};
use shardtree::{RetentionFlags, ShardTree};
use std::fmt::Debug;

use crate::{ByteArray, ToArray, TryByteArray, TryFromArray};

use super::TreeNode;

const SER_V1: u8 = 1;

const NIL_TAG: u8 = 0;
const LEAF_TAG: u8 = 1;
const PARENT_TAG: u8 = 2;

#[serde_as]
#[derive(Serialize, Deserialize)]
#[serde(remote = "Address")]
pub(crate) struct TreeAddressDef {
    #[serde_as(as = "FromInto<u8>")]
    #[serde(getter = "Address::level")]
    level: Level,
    #[serde(getter = "Address::index")]
    index: u64,
}

pub struct MemoryShardTreeDef;
impl<H, C, const DEPTH: u8, const SHARD_HEIGHT: u8>
    SerializeAs<ShardTree<MemoryShardStore<H, C>, DEPTH, SHARD_HEIGHT>> for MemoryShardTreeDef
where
    H: TreeNode<32>,
    C: Ord + Clone + Debug + From<u32> + Into<u32>,
{
    fn serialize_as<S>(
        value: &ShardTree<MemoryShardStore<H, C>, DEPTH, SHARD_HEIGHT>,
        serializer: S,
    ) -> Result<S::Ok, S::Error>
    where
        S: serde::Serializer,
    {
        #[serde_as]
        #[derive(Serialize)]
        struct ShardTreeSer<'a, H: TreeNode<32>, C: Ord + Clone + Debug + From<u32> + Into<u32>> {
            #[serde_as(as = "&'a MemoryShardStoreDef")]
            store: &'a MemoryShardStore<H, C>,
            max_checkpoints: usize,
        }
        ShardTreeSer {
            store: value.store(),
            max_checkpoints: value.max_checkpoints(),
        }
        .serialize(serializer)
    }
}

#[serde_as]
struct MemoryShardStoreDef;
impl<
        H: Clone + ToArray<u8, 32> + TryFromArray<u8, 32>,
        C: Ord + Clone + From<u32> + Into<u32>, // Most Cases this will be height
        T: ShardStore<H = H, CheckpointId = C>,
    > SerializeAs<T> for MemoryShardStoreDef
{
    fn serialize_as<S>(value: &T, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: serde::Serializer,
    {
        #[serde_as]
        #[derive(Serialize)]
        struct ShardStoreSer<
            'a,
            H: Clone + ToArray<u8, 32> + TryFromArray<u8, 32>,
            C: Ord + Clone + From<u32> + Into<u32>,
        > {
            #[serde_as(as = "&'a [LocatedPrunableTreeDef<H>]")]
            shards: &'a [LocatedPrunableTree<H>],
            #[serde_as(as = "BTreeMap<FromInto<u32>, CheckpointDef>")]
            checkpoints: BTreeMap<C, Checkpoint>,
            #[serde_as(as = "&'a PrunableTreeDef<32>")]
            cap: &'a PrunableTree<H>,
        }

        let shards = value
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
            .collect::<Result<Vec<_>, _>>()?;
        let mut checkpoints = BTreeMap::default();
        let checkpoint_count = value
            .checkpoint_count()
            .map_err(|_| serde::ser::Error::custom("Failed to get checkpoint count"))?;
        value
            .for_each_checkpoint(checkpoint_count, |checkpoint_id, checkpoint| {
                checkpoints.insert(checkpoint_id.clone(), checkpoint.clone());
                Ok(())
            })
            .map_err(serde::ser::Error::custom)?;

        ShardStoreSer {
            shards: &shards,
            checkpoints,
            cap: &value
                .get_cap()
                .map_err(|_| serde::ser::Error::custom("Failed to get cap"))?,
        }
        .serialize(serializer)
    }
}

impl<
        'de,
        H: Clone + ToArray<u8, 32> + TryFromArray<u8, 32>,
        C: Clone + Ord + From<u32> + Into<u32>,
    > serde_with::DeserializeAs<'de, MemoryShardStore<H, C>> for MemoryShardStoreDef
{
    fn deserialize_as<D>(deserializer: D) -> Result<MemoryShardStore<H, C>, D::Error>
    where
        D: Deserializer<'de>,
    {
        #[serde_as]
        #[derive(Deserialize)]
        struct MemoryShardStoreDe<
            H: Clone + ToArray<u8, 32> + TryFromArray<u8, 32>,
            C: Ord + Clone + From<u32> + Into<u32>,
        > {
            #[serde_as(as = "Vec<LocatedPrunableTreeDef<H>>")]
            shards: Vec<LocatedPrunableTree<H>>,
            #[serde_as(as = "BTreeMap<FromInto<u32>, CheckpointDef>")]
            checkpoints: BTreeMap<C, Checkpoint>,
            #[serde_as(as = "PrunableTreeDef<32>")]
            cap: PrunableTree<H>,
        }
        let de_store = MemoryShardStoreDe::<H, C>::deserialize(deserializer)?;
        let mut store = MemoryShardStore::empty();
        de_store.shards.into_iter().try_for_each(|shard| {
            store
                .put_shard(shard)
                .map_err(|_e| serde::de::Error::custom("Failed to put shard into store"))
        })?;
        store
            .put_cap(de_store.cap)
            .map_err(|_e| serde::de::Error::custom("Failed to put cap into store"))?;
        de_store
            .checkpoints
            .into_iter()
            .try_for_each(|(checkpoint_id, checkpoint)| {
                store
                    .add_checkpoint(checkpoint_id, checkpoint)
                    .map_err(|_e| serde::de::Error::custom("Failed to add checkpoint to store"))
            })?;

        Ok(store)
    }
}

impl<'de, H, C, const DEPTH: u8, const SHARD_HEIGHT: u8>
    DeserializeAs<'de, ShardTree<MemoryShardStore<H, C>, DEPTH, SHARD_HEIGHT>>
    for MemoryShardTreeDef
where
    H: TreeNode<32>,
    C: Ord + Clone + Debug + From<u32> + Into<u32>,
{
    fn deserialize_as<D>(
        deserializer: D,
    ) -> Result<ShardTree<MemoryShardStore<H, C>, DEPTH, SHARD_HEIGHT>, D::Error>
    where
        D: Deserializer<'de>,
    {
        #[serde_as]
        #[derive(Deserialize)]
        struct ShardTreeDe<H: TreeNode<32>, C: Ord + Clone + Debug + From<u32> + Into<u32>> {
            #[serde_as(as = "MemoryShardStoreDef")]
            store: MemoryShardStore<H, C>,
            max_checkpoints: usize,
        }
        let ShardTreeDe {
            store,
            max_checkpoints,
        } = ShardTreeDe::<H, C>::deserialize(deserializer)?;
        Ok(ShardTree::new(store, max_checkpoints))
    }
}
struct PrunableTreeDef<const N: usize>;
// This is copied from zcash_client_backend/src/serialization/shardtree.rs
impl<H: ToArray<u8, N>, const N: usize> SerializeAs<PrunableTree<H>> for PrunableTreeDef<N> {
    fn serialize_as<S>(value: &PrunableTree<H>, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: Serializer,
    {
        fn serialize_inner<H: ToArray<u8, N>, const N: usize, S>(
            tree: &PrunableTree<H>,
            state: &mut S::SerializeSeq,
        ) -> Result<(), S::Error>
        where
            S: Serializer,
        {
            match tree.deref() {
                Node::Parent { ann, left, right } => {
                    state.serialize_element(&PARENT_TAG)?;
                    state.serialize_element(
                        &ann.as_deref()
                            .map(ToArray::<u8, N>::to_array)
                            .map(ByteArray),
                    )?;
                    serialize_inner::<H, N, S>(left, state)?;
                    serialize_inner::<H, N, S>(right, state)?;
                    Ok(())
                }
                Node::Leaf { value } => {
                    state.serialize_element(&LEAF_TAG)?;
                    state.serialize_element(&ByteArray(value.0.to_array()))?;
                    state.serialize_element(&value.1.bits())?;
                    Ok(())
                }
                Node::Nil => {
                    state.serialize_element(&NIL_TAG)?;
                    Ok(())
                }
            }
        }

        let mut state = serializer.serialize_seq(None)?;
        state.serialize_element(&SER_V1)?;
        serialize_inner::<H, N, S>(value, &mut state)?;
        state.end()
    }
}
impl<'de, H: TryFromArray<u8, N>, const N: usize> DeserializeAs<'de, PrunableTree<H>>
    for PrunableTreeDef<N>
{
    fn deserialize_as<D>(deserializer: D) -> Result<PrunableTree<H>, D::Error>
    where
        D: Deserializer<'de>,
    {
        struct Visitor<H, const N: usize>(std::marker::PhantomData<H>);
        impl<H, const N: usize> Visitor<H, N> {
            fn new() -> Self {
                Self(std::marker::PhantomData)
            }
        }
        impl<'de, H: TryFromArray<u8, N>, const N: usize> serde::de::Visitor<'de> for Visitor<H, N> {
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
                let tree = deserialize_inner::<H, N, A>(&mut seq)?;
                Ok(tree)
            }
        }
        fn deserialize_inner<'de, H: TryFromArray<u8, N>, const N: usize, A>(
            seq: &mut A,
        ) -> Result<PrunableTree<H>, A::Error>
        where
            A: serde::de::SeqAccess<'de>,
        {
            // TODO: Is this right? We explicitly serialize the nil tag which isnt technically conventional
            let tag = seq.next_element()?.unwrap_or(NIL_TAG);

            match tag {
                PARENT_TAG => {
                    let ann = seq
                        .next_element::<Option<TryByteArray<N>>>()?
                        .ok_or_else(|| {
                            serde::de::Error::custom("Read parent tag but failed to read node")
                        })?
                        .map(|x| H::try_from_array(x.0).map(Arc::new))
                        .transpose()
                        .map_err(serde::de::Error::custom)?;

                    let left = deserialize_inner::<H, N, A>(seq)?;
                    let right = deserialize_inner::<H, N, A>(seq)?;
                    Ok(PrunableTree::parent(ann, left, right))
                }
                LEAF_TAG => {
                    let value = H::try_from_array(
                        seq.next_element::<ByteArray<N>>()?
                            .ok_or_else(|| {
                                serde::de::Error::custom("Read leaf tag but failed to read value")
                            })?
                            .0,
                    )
                    .map_err(serde::de::Error::custom)?;

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
        deserializer.deserialize_seq(Visitor::<H, N>::new())
    }
}

#[serde_as]
#[derive(Serialize, Deserialize)]
#[serde(remote = "LocatedPrunableTree")]
struct LocatedPrunableTreeDef<H: ToArray<u8, 32> + TryFromArray<u8, 32>> {
    #[serde_as(as = "TreeAddressDef")]
    #[serde(getter = "LocatedPrunableTree::root_addr")]
    pub root_addr: incrementalmerkletree::Address,
    #[serde_as(as = "PrunableTreeDef<32>")]
    #[serde(getter = "LocatedPrunableTree::root")]
    pub root: PrunableTree<H>,
}

#[serde_as]
#[derive(Serialize, Deserialize)]
#[serde(remote = "shardtree::store::TreeState")]
enum TreeStateDef {
    Empty,
    AtPosition(#[serde_as(as = "FromInto<u64>")] Position),
}

#[serde_as]
#[derive(Serialize, Deserialize)]
#[serde(remote = "shardtree::store::Checkpoint")]
struct CheckpointDef {
    #[serde_as(as = "TreeStateDef")]
    #[serde(getter = "shardtree::store::Checkpoint::tree_state")]
    pub tree_state: TreeState,
    #[serde_as(as = "BTreeSet<FromInto<u64>>")]
    #[serde(getter = "Checkpoint::marks_removed")]
    pub marks_removed: BTreeSet<Position>,
}

// BOILERPLATE: Trivial conversions between types and the trivial implementations of SerializeAs and DeserializeAs

impl From<CheckpointDef> for Checkpoint {
    fn from(def: CheckpointDef) -> Checkpoint {
        Checkpoint::from_parts(def.tree_state, def.marks_removed)
    }
}
impl SerializeAs<Checkpoint> for CheckpointDef {
    fn serialize_as<S>(value: &Checkpoint, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: serde::Serializer,
    {
        CheckpointDef::serialize(value, serializer)
    }
}
impl<'de> serde_with::DeserializeAs<'de, Checkpoint> for CheckpointDef {
    fn deserialize_as<D>(deserializer: D) -> Result<Checkpoint, D::Error>
    where
        D: Deserializer<'de>,
    {
        CheckpointDef::deserialize(deserializer)
    }
}
impl From<TreeAddressDef> for incrementalmerkletree::Address {
    fn from(def: TreeAddressDef) -> incrementalmerkletree::Address {
        incrementalmerkletree::Address::from_parts(def.level, def.index)
    }
}
impl SerializeAs<incrementalmerkletree::Address> for TreeAddressDef {
    fn serialize_as<S>(
        value: &incrementalmerkletree::Address,
        serializer: S,
    ) -> Result<S::Ok, S::Error>
    where
        S: serde::Serializer,
    {
        TreeAddressDef::serialize(value, serializer)
    }
}
impl<'de> serde_with::DeserializeAs<'de, incrementalmerkletree::Address> for TreeAddressDef {
    fn deserialize_as<D>(deserializer: D) -> Result<incrementalmerkletree::Address, D::Error>
    where
        D: Deserializer<'de>,
    {
        TreeAddressDef::deserialize(deserializer)
    }
}
impl SerializeAs<TreeState> for TreeStateDef {
    fn serialize_as<S>(value: &TreeState, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: serde::Serializer,
    {
        TreeStateDef::serialize(value, serializer)
    }
}
impl<'de> DeserializeAs<'de, TreeState> for TreeStateDef {
    fn deserialize_as<D>(deserializer: D) -> Result<TreeState, D::Error>
    where
        D: Deserializer<'de>,
    {
        TreeStateDef::deserialize(deserializer)
    }
}
impl<H: ToArray<u8, 32> + TryFromArray<u8, 32>> From<LocatedPrunableTreeDef<H>>
    for LocatedPrunableTree<H>
{
    fn from(def: LocatedPrunableTreeDef<H>) -> LocatedPrunableTree<H> {
        LocatedPrunableTree::from_parts(def.root_addr, def.root)
    }
}
impl<H: ToArray<u8, 32> + TryFromArray<u8, 32>> SerializeAs<LocatedPrunableTree<H>>
    for LocatedPrunableTreeDef<H>
{
    fn serialize_as<S>(value: &LocatedPrunableTree<H>, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: serde::Serializer,
    {
        LocatedPrunableTreeDef::serialize(value, serializer)
    }
}
impl<'de, H: ToArray<u8, 32> + TryFromArray<u8, 32>>
    serde_with::DeserializeAs<'de, LocatedPrunableTree<H>> for LocatedPrunableTreeDef<H>
{
    fn deserialize_as<D>(deserializer: D) -> Result<LocatedPrunableTree<H>, D::Error>
    where
        D: Deserializer<'de>,
    {
        LocatedPrunableTreeDef::deserialize(deserializer)
    }
}

#[cfg(test)]
mod tests {
    use crate::FromArray;

    use super::*;
    use ciborium::de::from_reader;
    use ciborium::ser::into_writer;
    use incrementalmerkletree::frontier::testing::{arb_test_node, TestNode};
    use proptest::prelude::*;
    use serde::{Deserialize, Serialize};
    use serde_with::serde_as;
    use shardtree::testing::arb_prunable_tree;
    use std::io::Cursor;

    #[serde_as]
    #[derive(Serialize, Deserialize, PartialEq, Eq, Clone, Debug)]
    struct PropPrunableTreeWrapper<H: Clone + ToArray<u8, 8> + TryFromArray<u8, 8>> {
        #[serde_as(as = "PrunableTreeDef<8>")]
        pub tree: PrunableTree<H>,
    }

    impl ToArray<u8, 8> for TestNode {
        fn to_array(&self) -> [u8; 8] {
            self.0.to_le_bytes()
        }
    }
    impl FromArray<u8, 8> for TestNode {
        fn from_array(bytes: [u8; 8]) -> Self {
            TestNode(u64::from_le_bytes(bytes))
        }
    }
    proptest! {
        #[test]
        fn check_shard_roundtrip(
            tree in arb_prunable_tree(arb_test_node(), 8, 32)
        ) {
            let mut tree_data = vec![];
            let tree = PropPrunableTreeWrapper { tree };
            into_writer(&tree, &mut tree_data).unwrap();

            let cursor = Cursor::new(tree_data);
            let tree_result = from_reader(cursor).unwrap();
            assert_eq!(tree, tree_result);
        }
    }
}
