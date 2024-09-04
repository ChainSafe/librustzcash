use crate::serialization::*;
use std::{
    cmp::Ordering,
    collections::{HashMap, HashSet},
};

use serde::{Deserialize, Serialize};
use serde_with::serde_as;
use serde_with::FromInto;
use serde_with::Seq;
use serde_with::SetPreventDuplicates;
use zcash_client_backend::wallet::NoteId;
use zcash_primitives::{block::BlockHash, consensus::BlockHeight, transaction::TxId};
use zcash_protocol::memo::MemoBytes;
/// Internal wallet representation of a Block.
#[serde_as]
#[derive(Serialize, Deserialize)]
pub(crate) struct MemoryWalletBlock {
    #[serde_as(as = "FromInto<u32>")]
    pub(crate) height: BlockHeight,
    #[serde_as(as = "BlockHashWrapper")]
    pub(crate) hash: BlockHash,
    pub(crate) block_time: u32,
    // Just the transactions that involve an account in this wallet
    #[serde_as(as = "SetPreventDuplicates<TxIdWrapper>")]
    pub(crate) _transactions: HashSet<TxId>,
    #[serde_as(as = "Seq<(NoteIdWrapper, MemoBytesWrapper)>")]
    pub(crate) _memos: HashMap<NoteId, MemoBytes>,
    pub(crate) sapling_commitment_tree_size: Option<u32>,
    pub(crate) _sapling_output_count: Option<u32>,
    #[cfg(feature = "orchard")]
    pub(crate) orchard_commitment_tree_size: Option<u32>,
    #[cfg(feature = "orchard")]
    pub(crate) _orchard_action_count: Option<u32>,
}

impl PartialEq for MemoryWalletBlock {
    fn eq(&self, other: &Self) -> bool {
        (self.height, self.block_time) == (other.height, other.block_time)
    }
}

impl Eq for MemoryWalletBlock {}

impl PartialOrd for MemoryWalletBlock {
    fn partial_cmp(&self, other: &Self) -> Option<Ordering> {
        Some((self.height, self.block_time).cmp(&(other.height, other.block_time)))
    }
}

impl Ord for MemoryWalletBlock {
    fn cmp(&self, other: &Self) -> Ordering {
        (self.height, self.block_time).cmp(&(other.height, other.block_time))
    }
}

mod serialization {}
