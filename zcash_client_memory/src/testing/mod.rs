use std::collections::BTreeMap;

use zcash_client_backend::{
    data_api::{
        chain::BlockSource,
        testing::{DataStoreFactory, Reset, TestCache, TestState},
    },
    proto::compact_formats::CompactBlock,
};
use zcash_protocol::local_consensus::LocalNetwork;

use crate::{Account, AccountId, Error, MemBlockCache, MemoryWalletDb};

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

impl TestCache for MemBlockCache {
    type BsError = ();
    type BlockSource = MemBlockCache;
    type InsertResult = ();

    fn block_source(&self) -> &Self::BlockSource {
        self
    }

    fn insert(&mut self, cb: &CompactBlock) -> Self::InsertResult {
        self.0.insert(cb.height().into(), cb.clone());
    }
}

impl<P> Reset for MemoryWalletDb<P>
where
    P: zcash_primitives::consensus::Parameters + Clone,
{
    type Handle = ();

    fn reset<C>(st: &mut TestState<C, Self, LocalNetwork>) -> Self::Handle {
        let new_wallet = MemoryWalletDb::new(st.wallet().params.clone(), 100);
        let _ = std::mem::replace(st.wallet_mut(), new_wallet);
    }
}
