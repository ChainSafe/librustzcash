use std::collections::BTreeMap;

use zcash_client_backend::{
    data_api::{
        chain::BlockSource,
        testing::{DataStoreFactory, NoteCommitments, TestCache},
    },
    proto::compact_formats::CompactBlock,
};
use zcash_protocol::local_consensus::LocalNetwork;

use crate::{Account, AccountId, Error, MemoryWalletDb};

pub mod pool;

/// A test data store factory for in-memory databases
/// Very simple implementation just creates a new MemoryWalletDb
pub(crate) struct TestMemDbFactory;

impl DataStoreFactory for TestMemDbFactory {
    type Error = ();
    type AccountId = AccountId;
    type Account = Account;
    type DsError = Error;
    type DataStore = MemoryWalletDb<LocalNetwork>;

    fn new_data_store(&self, network: LocalNetwork) -> Result<Self::DataStore, Self::Error> {
        Ok(MemoryWalletDb::new(network, 100))
    }
}

/// A block cache for testing. Just holds blocks in a map
pub(crate) struct MemBlockCache(BTreeMap<u64, CompactBlock>);

impl MemBlockCache {
    pub(crate) fn new() -> Self {
        MemBlockCache(BTreeMap::new())
    }
}

impl BlockSource for MemBlockCache {
    type Error = ();

    fn with_blocks<F, WalletErrT>(
        &self,
        from_height: Option<zcash_protocol::consensus::BlockHeight>,
        limit: Option<usize>,
        mut with_block: F,
    ) -> Result<(), zcash_client_backend::data_api::chain::error::Error<WalletErrT, Self::Error>>
    where
        F: FnMut(
            CompactBlock,
        ) -> Result<
            (),
            zcash_client_backend::data_api::chain::error::Error<WalletErrT, Self::Error>,
        >,
    {
        let block_iter = self
            .0
            .iter()
            .filter(|(_, cb)| {
                if let Some(from_height) = from_height {
                    cb.height() >= from_height
                } else {
                    true
                }
            })
            .take(limit.unwrap_or(usize::MAX));

        for (_, cb) in block_iter {
            with_block(cb.clone())?;
        }
        Ok(())
    }
}

impl TestCache for MemBlockCache {
    type BsError = ();
    type BlockSource = MemBlockCache;
    type InsertResult = ();

    fn block_source(&self) -> &Self::BlockSource {
        &self
    }

    fn insert(&mut self, cb: &CompactBlock) -> Self::InsertResult {
        self.0.insert(cb.height().into(), cb.clone());
        ()
    }
}
