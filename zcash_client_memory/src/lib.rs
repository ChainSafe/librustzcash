#![allow(dead_code)]
use incrementalmerkletree::{Address, Level, Marking, Position, Retention};
use scanning::ScanQueue;

use shardtree::{
    store::{memory::MemoryShardStore, Checkpoint, ShardStore},
    LocatedPrunableTree, PrunableTree, ShardTree,
};
use std::{cmp::min, io::Cursor, usize};
use std::{
    collections::{btree_map::Entry, BTreeMap, BTreeSet},
    num::NonZeroU32,
    ops::{Range, RangeInclusive},
};
use transparent::{
    TransparentReceivedOutputSpends, TransparentReceivedOutputs, TransparentSpendCache,
};
use zcash_keys::keys::UnifiedFullViewingKey;
use zcash_protocol::{
    consensus::{self, NetworkUpgrade},
    ShieldedProtocol,
};

use zip32::fingerprint::SeedFingerprint;

use zcash_client_backend::{
    serialization::shardtree::{read_shard, write_shard},
    wallet::WalletTransparentOutput,
};
use zcash_primitives::{
    consensus::BlockHeight,
    legacy::TransparentAddress,
    transaction::{components::OutPoint, TxId},
};

use zcash_client_backend::data_api::{GAP_LIMIT, SAPLING_SHARD_HEIGHT};
use zcash_client_backend::{
    data_api::{
        scanning::{ScanPriority, ScanRange},
        Account as _, AccountBirthday, AccountPurpose, AccountSource, InputSource, Ratio,
        TransactionStatus, WalletRead,
    },
    wallet::{NoteId, WalletSaplingOutput},
};

#[cfg(feature = "orchard")]
use zcash_client_backend::{data_api::ORCHARD_SHARD_HEIGHT, wallet::WalletOrchardOutput};

pub use crate::error::Error;
pub mod error;
pub mod input_source;
pub mod types;
pub mod wallet_commitment_trees;
pub mod wallet_read;
pub mod wallet_write;
pub(crate) use types::*;
pub mod block_source;
pub use block_source::*;

pub use types::MemoryWalletDb;

pub mod proto {
    pub mod memwallet {
        include!(concat!(env!("OUT_DIR"), "/memwallet.rs"));
    }
}

#[cfg(test)]
pub mod testing;

/// The maximum number of blocks the wallet is allowed to rewind. This is
/// consistent with the bound in zcashd, and allows block data deeper than
/// this delta from the chain tip to be pruned.
pub(crate) const PRUNING_DEPTH: u32 = 100;

/// The number of blocks to verify ahead when the chain tip is updated.
pub(crate) const VERIFY_LOOKAHEAD: u32 = 10;
