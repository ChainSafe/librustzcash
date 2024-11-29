use std::{
    cmp::Ordering,
    collections::{HashMap, HashSet},
};

use zcash_client_backend::wallet::NoteId;
use zcash_primitives::{block::BlockHash, consensus::BlockHeight, transaction::TxId};
use zcash_protocol::memo::MemoBytes;
/// Internal wallet representation of a Block.
#[derive(Clone, Debug, PartialEq)]
pub(crate) struct MemoryWalletBlock {
    pub(crate) height: BlockHeight,
    pub(crate) hash: BlockHash,
    pub(crate) block_time: u32,
    // Just the transactions that involve an account in this wallet
    pub(crate) _transactions: HashSet<TxId>,
    pub(crate) _memos: HashMap<NoteId, MemoBytes>,
    pub(crate) sapling_commitment_tree_size: Option<u32>,
    pub(crate) sapling_output_count: Option<u32>,
    #[cfg(feature = "orchard")]
    pub(crate) orchard_commitment_tree_size: Option<u32>,
    #[cfg(feature = "orchard")]
    pub(crate) orchard_action_count: Option<u32>,
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

mod serialization {
    use super::*;
    use crate::proto::memwallet as proto;

    impl From<MemoryWalletBlock> for proto::WalletBlock {
        fn from(block: MemoryWalletBlock) -> Self {
            Self {
                height: block.height.into(),
                hash: block.hash.0.to_vec(),
                block_time: block.block_time,
                transactions: block
                    ._transactions
                    .into_iter()
                    .map(|txid| txid.as_ref().to_vec())
                    .collect(),
                memos: block
                    ._memos
                    .into_iter()
                    .map(|(note_id, memo)| proto::Memo {
                        note_id: Some(note_id.into()),
                        memo: memo.as_array().to_vec(),
                    })
                    .collect(),
                sapling_commitment_tree_size: block.sapling_commitment_tree_size,
                sapling_output_count: block.sapling_output_count,
                #[cfg(feature = "orchard")]
                orchard_commitment_tree_size: block.orchard_commitment_tree_size,
                #[cfg(not(feature = "orchard"))]
                orchard_commitment_tree_size: None,
                #[cfg(feature = "orchard")]
                orchard_action_count: block.orchard_action_count,
                #[cfg(not(feature = "orchard"))]
                orchard_action_count: None,
            }
        }
    }

    impl From<proto::WalletBlock> for MemoryWalletBlock {
        fn from(block: proto::WalletBlock) -> Self {
            Self {
                height: block.height.into(),
                hash: BlockHash(block.hash.try_into().unwrap()),
                block_time: block.block_time,
                _transactions: block
                    .transactions
                    .into_iter()
                    .map(|txid| TxId::from_bytes(txid.try_into().unwrap()))
                    .collect(),
                _memos: block
                    .memos
                    .into_iter()
                    .map(|memo| {
                        let note_id = memo.note_id.unwrap();
                        (
                            NoteId::new(
                                note_id.tx_id.clone().unwrap().into(),
                                match note_id.pool() {
                                    proto::PoolType::ShieldedSapling => {
                                        zcash_protocol::ShieldedProtocol::Sapling
                                    }
                                    #[cfg(feature = "orchard")]
                                    proto::PoolType::ShieldedOrchard => {
                                        zcash_protocol::ShieldedProtocol::Orchard
                                    }
                                    _ => unreachable!(),
                                },
                                note_id.output_index as u16,
                            ),
                            MemoBytes::from_bytes(&memo.memo).unwrap(),
                        )
                    })
                    .collect(),
                sapling_commitment_tree_size: block.sapling_commitment_tree_size,
                sapling_output_count: block.sapling_output_count,
                #[cfg(feature = "orchard")]
                orchard_commitment_tree_size: block.orchard_commitment_tree_size,
                #[cfg(feature = "orchard")]
                orchard_action_count: block.orchard_action_count,
            }
        }
    }

    #[cfg(test)]
    mod tests {
        use super::*;
        use crate::proto::memwallet as proto;
        use pretty_assertions::assert_eq;
        use zcash_primitives::block::BlockHash;

        #[test]
        fn test_block_roundtrip() {
            let block = MemoryWalletBlock {
                height: 1.into(),
                hash: BlockHash([0; 32]),
                block_time: 2,
                _transactions: HashSet::new(),
                _memos: HashMap::new(),
                sapling_commitment_tree_size: Some(3),
                sapling_output_count: Some(4),
                #[cfg(feature = "orchard")]
                orchard_commitment_tree_size: Some(5),
                #[cfg(feature = "orchard")]
                orchard_action_count: Some(6),
            };

            let proto: proto::WalletBlock = block.clone().into();
            let recovered: MemoryWalletBlock = proto.clone().into();

            assert_eq!(block, recovered);
        }
    }
}
