use zcash_client_backend::data_api::testing::DataStoreFactory;
use zcash_protocol::local_consensus::LocalNetwork;

use crate::{AccountId, Account, Error, MemoryWalletDb};

pub(crate) struct TestDbFactory;

impl DataStoreFactory for TestDbFactory {
    type Error = ();
    type AccountId = AccountId;
    type Account = Account;
    type DsError = Error;
    type DataStore = MemoryWalletDb<LocalNetwork>;

    fn new_data_store(&self, network: LocalNetwork) -> Result<Self::DataStore, Self::Error> {
        Ok(MemoryWalletDb::new(network, 100))
    }
}
