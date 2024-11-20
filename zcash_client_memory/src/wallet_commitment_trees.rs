use std::convert::Infallible;

use incrementalmerkletree::Address;
use shardtree::{error::ShardTreeError, store::memory::MemoryShardStore, ShardTree};
#[cfg(feature = "orchard")]
use zcash_client_backend::data_api::ORCHARD_SHARD_HEIGHT;
use zcash_client_backend::data_api::{
    chain::CommitmentTreeRoot, WalletCommitmentTrees, SAPLING_SHARD_HEIGHT,
};
use zcash_primitives::consensus::BlockHeight;
use zcash_protocol::consensus;

use crate::MemoryWalletDb;

impl<P: consensus::Parameters> WalletCommitmentTrees for MemoryWalletDb<P> {
    type Error = Infallible;
    type SaplingShardStore<'a> = MemoryShardStore<sapling::Node, BlockHeight>;

    fn with_sapling_tree_mut<F, A, E>(&mut self, mut callback: F) -> Result<A, E>
    where
        for<'a> F: FnMut(
            &'a mut ShardTree<
                Self::SaplingShardStore<'a>,
                { sapling::NOTE_COMMITMENT_TREE_DEPTH },
                SAPLING_SHARD_HEIGHT,
            >,
        ) -> Result<A, E>,
        E: From<ShardTreeError<Infallible>>,
    {
        tracing::debug!("with_sapling_tree_mut");
        callback(&mut self.sapling_tree)
    }

    fn put_sapling_subtree_roots(
        &mut self,
        start_index: u64,
        roots: &[CommitmentTreeRoot<sapling::Node>],
    ) -> Result<(), ShardTreeError<Self::Error>> {
        tracing::debug!("put_sapling_subtree_roots");
        self.with_sapling_tree_mut(|t| {
            for (root, i) in roots.iter().zip(0u64..) {
                let root_addr = Address::from_parts(SAPLING_SHARD_HEIGHT.into(), start_index + i);
                t.insert(root_addr, *root.root_hash())?;
            }
            Ok::<_, ShardTreeError<Self::Error>>(())
        })?;

        // store the end block heights for each shard as well
        for (root, i) in roots.iter().zip(0u64..) {
            let root_addr = Address::from_parts(SAPLING_SHARD_HEIGHT.into(), start_index + i);
            self.sapling_tree_shard_end_heights
                .insert(root_addr, root.subtree_end_height());
        }

        Ok(())
    }

    #[cfg(feature = "orchard")]
    type OrchardShardStore<'a> = MemoryShardStore<orchard::tree::MerkleHashOrchard, BlockHeight>;

    #[cfg(feature = "orchard")]
    fn with_orchard_tree_mut<F, A, E>(&mut self, mut callback: F) -> Result<A, E>
    where
        for<'a> F: FnMut(
            &'a mut ShardTree<
                Self::OrchardShardStore<'a>,
                { ORCHARD_SHARD_HEIGHT * 2 },
                ORCHARD_SHARD_HEIGHT,
            >,
        ) -> Result<A, E>,
        E: From<ShardTreeError<Self::Error>>,
    {
        tracing::debug!("with_orchard_tree_mut");
        callback(&mut self.orchard_tree)
    }

    /// Adds a sequence of note commitment tree subtree roots to the data store.
    #[cfg(feature = "orchard")]
    fn put_orchard_subtree_roots(
        &mut self,
        start_index: u64,
        roots: &[CommitmentTreeRoot<orchard::tree::MerkleHashOrchard>],
    ) -> Result<(), ShardTreeError<Self::Error>> {
        tracing::debug!("put_orchard_subtree_roots");
        self.with_orchard_tree_mut(|t| {
            for (root, i) in roots.iter().zip(0u64..) {
                let root_addr = Address::from_parts(ORCHARD_SHARD_HEIGHT.into(), start_index + i);
                t.insert(root_addr, *root.root_hash())?;
            }
            Ok::<_, ShardTreeError<Self::Error>>(())
        })?;

        // store the end block heights for each shard as well
        for (root, i) in roots.iter().zip(0u64..) {
            let root_addr = Address::from_parts(SAPLING_SHARD_HEIGHT.into(), start_index + i);
            self.orchard_tree_shard_end_heights
                .insert(root_addr, root.subtree_end_height());
        }

        Ok(())
    }
}
