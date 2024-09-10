use zcash_client_backend::data_api::testing::pool::ShieldedPoolTester;

use crate::testing::{TestMemDbFactory, MemBlockCache};

pub(crate) fn send_single_step_proposed_transfer<T: ShieldedPoolTester>() {
    zcash_client_backend::data_api::testing::pool::send_single_step_proposed_transfer::<T>(
        TestMemDbFactory,
        MemBlockCache::new(),
    )
}

#[cfg(test)]
mod sapling_tests {
    use zcash_client_backend::data_api::testing::sapling::SaplingPoolTester;

    #[test]
    fn send_single_step_proposed_transfer() {
        crate::testing::pool::send_single_step_proposed_transfer::<SaplingPoolTester>()
    }
}

#[cfg(test)]
#[cfg(feature = "orchard")]
mod orchard_tests {
    use zcash_client_backend::data_api::testing::orchard::OrchardPoolTester;

    #[test]
    fn send_single_step_proposed_transfer() {
        crate::testing::pool::send_single_step_proposed_transfer::<OrchardPoolTester>()
    }
}
