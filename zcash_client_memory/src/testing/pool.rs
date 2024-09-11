use zcash_client_backend::data_api::testing::pool::ShieldedPoolTester;

use crate::testing::{MemBlockCache, TestMemDbFactory};

pub(crate) fn send_single_step_proposed_transfer<T: ShieldedPoolTester>() {
    zcash_client_backend::data_api::testing::pool::send_single_step_proposed_transfer::<T>(
        TestMemDbFactory,
        MemBlockCache::new(),
    )
}

#[cfg(feature = "transparent-inputs")]
pub(crate) fn send_multi_step_proposed_transfer<T: ShieldedPoolTester>() {
    zcash_client_backend::data_api::testing::pool::send_multi_step_proposed_transfer::<T, _>(
        TestMemDbFactory,
        MemBlockCache::new(),
    )
}

#[cfg(feature = "transparent-inputs")]
pub(crate) fn proposal_fails_if_not_all_ephemeral_outputs_consumed<T: ShieldedPoolTester>() {
    zcash_client_backend::data_api::testing::pool::proposal_fails_if_not_all_ephemeral_outputs_consumed::<T>(
        TestMemDbFactory,
        MemBlockCache::new(),
    )
}

#[allow(deprecated)]
pub(crate) fn create_to_address_fails_on_incorrect_usk<T: ShieldedPoolTester>() {
    zcash_client_backend::data_api::testing::pool::create_to_address_fails_on_incorrect_usk::<T>(
        TestMemDbFactory,
    )
}

#[allow(deprecated)]
pub(crate) fn proposal_fails_with_no_blocks<T: ShieldedPoolTester>() {
    zcash_client_backend::data_api::testing::pool::proposal_fails_with_no_blocks::<T, _>(
        TestMemDbFactory,
    )
}

pub(crate) fn spend_fails_on_unverified_notes<T: ShieldedPoolTester>() {
    zcash_client_backend::data_api::testing::pool::spend_fails_on_unverified_notes::<T>(
        TestMemDbFactory,
        MemBlockCache::new(),
    )
}

pub(crate) fn spend_fails_on_locked_notes<T: ShieldedPoolTester>() {
    zcash_client_backend::data_api::testing::pool::spend_fails_on_locked_notes::<T>(
        TestMemDbFactory,
        MemBlockCache::new(),
    )
}

pub(crate) fn ovk_policy_prevents_recovery_from_chain<T: ShieldedPoolTester>() {
    zcash_client_backend::data_api::testing::pool::ovk_policy_prevents_recovery_from_chain::<T, _>(
        TestMemDbFactory,
        MemBlockCache::new(),
    )
}

pub(crate) fn spend_succeeds_to_t_addr_zero_change<T: ShieldedPoolTester>() {
    zcash_client_backend::data_api::testing::pool::spend_succeeds_to_t_addr_zero_change::<T>(
        TestMemDbFactory,
        MemBlockCache::new(),
    )
}

pub(crate) fn change_note_spends_succeed<T: ShieldedPoolTester>() {
    zcash_client_backend::data_api::testing::pool::change_note_spends_succeed::<T, _>(
        TestMemDbFactory,
        MemBlockCache::new(),
    )
}

// TODO: Implement reset for memdb
// pub(crate) fn external_address_change_spends_detected_in_restore_from_seed<
//     T: ShieldedPoolTester,
// >() {
//     zcash_client_backend::data_api::testing::pool::external_address_change_spends_detected_in_restore_from_seed::<T, _>(
//         TestMemDbFactory,
//         MemBlockCache::new(),
//     )
// }

#[allow(dead_code)]
pub(crate) fn zip317_spend<T: ShieldedPoolTester>() {
    zcash_client_backend::data_api::testing::pool::zip317_spend::<T>(
        TestMemDbFactory,
        MemBlockCache::new(),
    )
}

#[cfg(feature = "transparent-inputs")]
pub(crate) fn shield_transparent<T: ShieldedPoolTester>() {
    zcash_client_backend::data_api::testing::pool::shield_transparent::<T, _>(
        TestMemDbFactory,
        MemBlockCache::new(),
    )
}

pub(crate) fn birthday_in_anchor_shard<T: ShieldedPoolTester>() {
    zcash_client_backend::data_api::testing::pool::birthday_in_anchor_shard::<T>(
        TestMemDbFactory,
        MemBlockCache::new(),
    )
}

pub(crate) fn checkpoint_gaps<T: ShieldedPoolTester>() {
    zcash_client_backend::data_api::testing::pool::checkpoint_gaps::<T>(
        TestMemDbFactory,
        MemBlockCache::new(),
    )
}

#[cfg(feature = "orchard")]
pub(crate) fn pool_crossing_required<T: ShieldedPoolTester, TT: ShieldedPoolTester>() {
    zcash_client_backend::data_api::testing::pool::pool_crossing_required::<T, TT>(
        TestMemDbFactory,
        MemBlockCache::new(),
    )
}

#[cfg(feature = "orchard")]
pub(crate) fn fully_funded_fully_private<T: ShieldedPoolTester, TT: ShieldedPoolTester>() {
    zcash_client_backend::data_api::testing::pool::fully_funded_fully_private::<T, TT>(
        TestMemDbFactory,
        MemBlockCache::new(),
    )
}

#[cfg(all(feature = "orchard", feature = "transparent-inputs"))]
pub(crate) fn fully_funded_send_to_t<T: ShieldedPoolTester, TT: ShieldedPoolTester>() {
    zcash_client_backend::data_api::testing::pool::fully_funded_send_to_t::<T, TT>(
        TestMemDbFactory,
        MemBlockCache::new(),
    )
}

#[cfg(feature = "orchard")]
pub(crate) fn multi_pool_checkpoint<T: ShieldedPoolTester, TT: ShieldedPoolTester>() {
    zcash_client_backend::data_api::testing::pool::multi_pool_checkpoint::<T, TT>(
        TestMemDbFactory,
        MemBlockCache::new(),
    )
}

#[cfg(feature = "orchard")]
pub(crate) fn multi_pool_checkpoints_with_pruning<T: ShieldedPoolTester, TT: ShieldedPoolTester>() {
    zcash_client_backend::data_api::testing::pool::multi_pool_checkpoints_with_pruning::<T, TT>(
        TestMemDbFactory,
        MemBlockCache::new(),
    )
}

pub(crate) fn valid_chain_states<T: ShieldedPoolTester>() {
    zcash_client_backend::data_api::testing::pool::valid_chain_states::<T>(
        TestMemDbFactory,
        MemBlockCache::new(),
    )
}

pub(crate) fn invalid_chain_cache_disconnected<T: ShieldedPoolTester>() {
    zcash_client_backend::data_api::testing::pool::invalid_chain_cache_disconnected::<T>(
        TestMemDbFactory,
        MemBlockCache::new(),
    )
}

pub(crate) fn data_db_truncation<T: ShieldedPoolTester>() {
    zcash_client_backend::data_api::testing::pool::data_db_truncation::<T, _>(
        TestMemDbFactory,
        MemBlockCache::new(),
    )
}

pub(crate) fn scan_cached_blocks_allows_blocks_out_of_order<T: ShieldedPoolTester>() {
    zcash_client_backend::data_api::testing::pool::scan_cached_blocks_allows_blocks_out_of_order::<T>(
        TestMemDbFactory,
        MemBlockCache::new(),
    )
}

pub(crate) fn scan_cached_blocks_finds_received_notes<T: ShieldedPoolTester>() {
    zcash_client_backend::data_api::testing::pool::scan_cached_blocks_finds_received_notes::<T, _>(
        TestMemDbFactory,
        MemBlockCache::new(),
    )
}

pub(crate) fn scan_cached_blocks_finds_change_notes<T: ShieldedPoolTester>() {
    zcash_client_backend::data_api::testing::pool::scan_cached_blocks_finds_change_notes::<T, _>(
        TestMemDbFactory,
        MemBlockCache::new(),
    )
}

pub(crate) fn scan_cached_blocks_detects_spends_out_of_order<T: ShieldedPoolTester>() {
    zcash_client_backend::data_api::testing::pool::scan_cached_blocks_detects_spends_out_of_order::<
        T,
        _,
    >(TestMemDbFactory, MemBlockCache::new())
}

#[cfg(test)]
mod sapling_tests {
    use zcash_client_backend::data_api::testing::sapling::SaplingPoolTester;
    #[cfg(feature = "orchard")]
    use zcash_client_backend::data_api::testing::orchard::OrchardPoolTester;
    use crate::testing;

    #[test]
    fn send_single_step_proposed_transfer() {
        testing::pool::send_single_step_proposed_transfer::<SaplingPoolTester>()
    }

    #[test]
    #[cfg(feature = "transparent-inputs")]
    fn send_multi_step_proposed_transfer() {
        testing::pool::send_multi_step_proposed_transfer::<SaplingPoolTester>()
    }

    #[test]
    #[cfg(feature = "transparent-inputs")]
    fn proposal_fails_if_not_all_ephemeral_outputs_consumed() {
        testing::pool::proposal_fails_if_not_all_ephemeral_outputs_consumed::<SaplingPoolTester>()
    }

    #[test]
    #[allow(deprecated)]
    fn create_to_address_fails_on_incorrect_usk() {
        testing::pool::create_to_address_fails_on_incorrect_usk::<SaplingPoolTester>()
    }

    #[test]
    #[allow(deprecated)]
    fn proposal_fails_with_no_blocks() {
        testing::pool::proposal_fails_with_no_blocks::<SaplingPoolTester>()
    }

    #[test]
    fn spend_fails_on_unverified_notes() {
        testing::pool::spend_fails_on_unverified_notes::<SaplingPoolTester>()
    }

    #[test]
    fn spend_fails_on_locked_notes() {
        testing::pool::spend_fails_on_locked_notes::<SaplingPoolTester>()
    }

    #[test]
    fn ovk_policy_prevents_recovery_from_chain() {
        testing::pool::ovk_policy_prevents_recovery_from_chain::<SaplingPoolTester>()
    }

    #[test]
    fn spend_succeeds_to_t_addr_zero_change() {
        testing::pool::spend_succeeds_to_t_addr_zero_change::<SaplingPoolTester>()
    }

    #[test]
    fn change_note_spends_succeed() {
        testing::pool::change_note_spends_succeed::<SaplingPoolTester>()
    }

    // #[test]
    // fn external_address_change_spends_detected_in_restore_from_seed() {
    //     testing::pool::external_address_change_spends_detected_in_restore_from_seed::<
    //         SaplingPoolTester,
    //     >()
    // }

    // #[test]
    // #[ignore] // FIXME: #1316 This requires support for dust outputs.
    // #[cfg(not(feature = "expensive-tests"))]
    // fn zip317_spend() {
    //     testing::pool::zip317_spend::<SaplingPoolTester>()
    // }

    #[test]
    #[cfg(feature = "transparent-inputs")]
    fn shield_transparent() {
        testing::pool::shield_transparent::<SaplingPoolTester>()
    }

    #[test]
    fn birthday_in_anchor_shard() {
        testing::pool::birthday_in_anchor_shard::<SaplingPoolTester>()
    }

    #[test]
    fn checkpoint_gaps() {
        testing::pool::checkpoint_gaps::<SaplingPoolTester>()
    }

    #[test]
    fn scan_cached_blocks_detects_spends_out_of_order() {
        testing::pool::scan_cached_blocks_detects_spends_out_of_order::<SaplingPoolTester>()
    }

    #[test]
    #[cfg(feature = "orchard")]
    fn pool_crossing_required() {
        testing::pool::pool_crossing_required::<SaplingPoolTester, OrchardPoolTester>()
    }

    #[test]
    #[cfg(feature = "orchard")]
    fn fully_funded_fully_private() {
        testing::pool::fully_funded_fully_private::<SaplingPoolTester, OrchardPoolTester>()
    }

    #[test]
    #[cfg(all(feature = "orchard", feature = "transparent-inputs"))]
    fn fully_funded_send_to_t() {
        testing::pool::fully_funded_send_to_t::<SaplingPoolTester, OrchardPoolTester>()
    }

    #[test]
    #[cfg(feature = "orchard")]
    fn multi_pool_checkpoint() {
        testing::pool::multi_pool_checkpoint::<SaplingPoolTester, OrchardPoolTester>()
    }

    #[test]
    #[cfg(feature = "orchard")]
    fn multi_pool_checkpoints_with_pruning() {
        testing::pool::multi_pool_checkpoints_with_pruning::<SaplingPoolTester, OrchardPoolTester>()
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
