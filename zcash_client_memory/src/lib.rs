#![allow(dead_code)]
use incrementalmerkletree::Address;
use scanning::ScanQueue;

use serde::{Deserialize, Serialize};
use serde_with::serde_as;
use shardtree::{
    store::{memory::MemoryShardStore, ShardStore as _},
    ShardTree,
};
use std::{
    collections::{btree_map::Entry, BTreeMap},
    num::NonZeroU32,
    ops::RangeInclusive,
};
use zcash_protocol::consensus;
use zip32::fingerprint::SeedFingerprint;

use zcash_primitives::{consensus::BlockHeight, transaction::TxId};

use zcash_client_backend::data_api::SAPLING_SHARD_HEIGHT;
use zcash_client_backend::{
    data_api::{Account as _, AccountSource, InputSource, TransactionStatus, WalletRead},
    wallet::{NoteId, WalletSaplingOutput},
};

#[cfg(feature = "orchard")]
use zcash_client_backend::{data_api::ORCHARD_SHARD_HEIGHT, wallet::WalletOrchardOutput};

use crate::error::Error;
mod error;
pub mod input_source;
pub mod types;
pub mod wallet_commitment_trees;
pub mod wallet_read;
pub mod wallet_write;
pub(crate) use types::*;

/// The maximum number of blocks the wallet is allowed to rewind. This is
/// consistent with the bound in zcashd, and allows block data deeper than
/// this delta from the chain tip to be pruned.
pub(crate) const PRUNING_DEPTH: u32 = 100;

/// The number of blocks to verify ahead when the chain tip is updated.
pub(crate) const VERIFY_LOOKAHEAD: u32 = 10;

use serde_with::{FromInto, TryFromInto};
use types::serialization::*;

/// The main in-memory wallet database. Implements all the traits needed to be used as a backend.
#[serde_as]
#[derive(Serialize)]
pub struct MemoryWalletDb<P: consensus::Parameters> {
    #[serde(skip_serializing)]
    params: P,
    #[serde(skip_serializing)]
    accounts: Accounts,
    #[serde_as(as = "BTreeMap<FromInto<u32>, _>")]
    blocks: BTreeMap<BlockHeight, MemoryWalletBlock>,
    tx_table: TransactionTable,
    received_notes: ReceivedNoteTable,
    received_note_spends: ReceievdNoteSpends,
    nullifiers: NullifierMap,

    tx_locator: TxLocatorMap,
    scan_queue: ScanQueue,
    #[serde(skip_serializing)]
    sapling_tree: ShardTree<
        MemoryShardStore<sapling::Node, BlockHeight>,
        { SAPLING_SHARD_HEIGHT * 2 },
        SAPLING_SHARD_HEIGHT,
    >,
    /// Stores the block height corresponding to the last note commitment in a shard
    #[serde_as(as = "BTreeMap<TreeAddressWrapper, FromInto<u32>>")]
    sapling_tree_shard_end_heights: BTreeMap<Address, BlockHeight>,

    #[cfg(feature = "orchard")]
    #[serde(skip_serializing)]
    orchard_tree: ShardTree<
        MemoryShardStore<orchard::tree::MerkleHashOrchard, BlockHeight>,
        { ORCHARD_SHARD_HEIGHT * 2 },
        ORCHARD_SHARD_HEIGHT,
    >,
    #[cfg(feature = "orchard")]
    /// Stores the block height corresponding to the last note commitment in a shard
    #[serde_as(as = "BTreeMap<TreeAddressWrapper, FromInto<u32>>")]
    orchard_tree_shard_end_heights: BTreeMap<Address, BlockHeight>,
}

impl<P: consensus::Parameters> MemoryWalletDb<P> {
    pub fn new(params: P, max_checkpoints: usize) -> Self {
        Self {
            accounts: Accounts::new(),
            params,
            blocks: BTreeMap::new(),
            sapling_tree: ShardTree::new(MemoryShardStore::empty(), max_checkpoints),
            sapling_tree_shard_end_heights: BTreeMap::new(),
            #[cfg(feature = "orchard")]
            orchard_tree: ShardTree::new(MemoryShardStore::empty(), max_checkpoints),
            #[cfg(feature = "orchard")]
            orchard_tree_shard_end_heights: BTreeMap::new(),
            tx_table: TransactionTable::new(),
            received_notes: ReceivedNoteTable::new(),
            nullifiers: NullifierMap::new(),
            tx_locator: TxLocatorMap::new(),
            received_note_spends: ReceievdNoteSpends::new(),
            scan_queue: ScanQueue::new(),
        }
    }

    pub(crate) fn get_received_notes(&self) -> &ReceivedNoteTable {
        &self.received_notes
    }

    // TODO: Update this if we switch from using a vec to store received notes to
    // someething with more efficient lookups
    pub(crate) fn get_received_note(&self, note_id: NoteId) -> Option<&ReceivedNote> {
        self.received_notes
            .0
            .iter()
            .find(|v| v.note_id() == note_id)
    }

    pub(crate) fn mark_sapling_note_spent(
        &mut self,
        nf: sapling::Nullifier,
        txid: TxId,
    ) -> Result<(), Error> {
        let note_id = self
            .received_notes
            .0
            .iter()
            .filter(|v| v.nullifier() == Some(&Nullifier::Sapling(nf)))
            .map(|v| v.note_id())
            .next()
            .ok_or_else(|| Error::NoteNotFound)?;
        self.received_note_spends.insert_spend(note_id, txid);
        Ok(())
    }

    /// Returns true if the note is in the spent notes table and the transaction that spent it is
    /// in the transaction table and has either been mined or can be mined in the future
    /// (i.e. it hasn't or will not expire)
    pub(crate) fn note_is_spent(
        &self,
        note: &ReceivedNote,
        min_confirmations: u32,
    ) -> Result<bool, Error> {
        let spend = self.received_note_spends.get(&note.note_id());

        let spent = match spend {
            Some(txid) => {
                let spending_tx = self
                    .tx_table
                    .get(txid)
                    .ok_or_else(|| Error::TransactionNotFound(*txid))?;
                match spending_tx.status() {
                    TransactionStatus::Mined(_height) => true,
                    TransactionStatus::TxidNotRecognized => unreachable!(),
                    TransactionStatus::NotInMainChain => {
                        // check the expiry
                        spending_tx.expiry_height().is_none() // no expiry, tx could be mined any time so we consider it spent
                            // expiry is in the future so it could still be mined
                            || spending_tx.expiry_height() > self.summary_height(min_confirmations)?
                    }
                }
            }
            None => false,
        };
        Ok(spent)
    }

    /// To be spendable a note must be:
    /// - unspent (obviously)
    /// - not dust (value > 5000 ZATs)
    /// - be associated with an account with a ufvk
    /// - have a recipient key scope
    /// - We know the nullifier
    /// - We know the commitment tree position
    /// - be in a block less than or equal to the anchor height
    /// - not be in the given exclude list
    ///
    /// Additionally the tree shard containing the node must not be in an unscanned range
    /// excluding ranges that start above the anchor height or end below the wallet birthday.
    /// This is determined by looking at the scan queue
    pub(crate) fn note_is_spendable(
        &self,
        note: &ReceivedNote,
        birthday_height: zcash_protocol::consensus::BlockHeight,
        anchor_height: zcash_protocol::consensus::BlockHeight,
        exclude: &[<MemoryWalletDb<P> as InputSource>::NoteRef],
    ) -> Result<bool, Error> {
        let note_account = self
            .get_account(note.account_id())?
            .ok_or_else(|| Error::AccountUnknown(note.account_id))?;
        let note_txn = self
            .tx_table
            .get(&note.txid())
            .ok_or_else(|| Error::TransactionNotFound(note.txid()))?;

        // TODO: Add the unscanned range check

        Ok(!self.note_is_spent(note, 0)?
            && note.note.value().into_u64() > 5000
            && note_account.ufvk().is_some()
            && note.recipient_key_scope.is_some()
            && note.nullifier().is_some()
            && note.commitment_tree_position.is_some()
            && note_txn.mined_height().is_some()
            && note_txn.mined_height().unwrap() <= anchor_height
            && !exclude.contains(&note.note_id()))
    }

    pub fn summary_height(&self, min_confirmations: u32) -> Result<Option<BlockHeight>, Error> {
        let chain_tip_height = match self.chain_height()? {
            Some(height) => height,
            None => return Ok(None),
        };
        let summary_height =
            (chain_tip_height + 1).saturating_sub(std::cmp::max(min_confirmations, 1));
        Ok(Some(summary_height))
    }

    #[cfg(feature = "orchard")]
    pub(crate) fn mark_orchard_note_spent(
        &mut self,
        nf: orchard::note::Nullifier,
        txid: TxId,
    ) -> Result<(), Error> {
        let note_id = self
            .received_notes
            .0
            .iter()
            .filter(|v| v.nullifier() == Some(&Nullifier::Orchard(nf)))
            .map(|v| v.note_id())
            .next()
            .ok_or_else(|| Error::NoteNotFound)?;
        self.received_note_spends.insert_spend(note_id, txid);
        Ok(())
    }

    pub(crate) fn max_zip32_account_index(
        &self,
        seed_fingerprint: &SeedFingerprint,
    ) -> Result<Option<zip32::AccountId>, Error> {
        Ok(self
            .accounts
            .iter()
            .filter_map(|(_, a)| match a.source() {
                AccountSource::Derived {
                    seed_fingerprint: sf,
                    account_index,
                } => {
                    if &sf == seed_fingerprint {
                        Some(account_index)
                    } else {
                        None
                    }
                }
                _ => None,
            })
            .max())
    }
    pub(crate) fn insert_received_sapling_note(
        &mut self,
        note_id: NoteId,
        output: &WalletSaplingOutput<AccountId>,
        spent_in: Option<TxId>,
    ) {
        self.received_notes
            .insert_received_note(ReceivedNote::from_wallet_sapling_output(note_id, output));
        if let Some(spent_in) = spent_in {
            self.received_note_spends.insert_spend(note_id, spent_in);
        }
    }
    #[cfg(feature = "orchard")]
    pub(crate) fn insert_received_orchard_note(
        &mut self,
        note_id: NoteId,
        output: &WalletOrchardOutput<AccountId>,
        spent_in: Option<TxId>,
    ) {
        self.received_notes
            .insert_received_note(ReceivedNote::from_wallet_orchard_output(note_id, output));
        if let Some(spent_in) = spent_in {
            self.received_note_spends.insert_spend(note_id, spent_in);
        }
    }
    pub(crate) fn insert_sapling_nullifier_map(
        &mut self,
        block_height: BlockHeight,
        new_entries: &[(TxId, u16, Vec<sapling::Nullifier>)],
    ) -> Result<(), Error> {
        for (txid, tx_index, nullifiers) in new_entries {
            match self.tx_locator.entry((block_height, *tx_index as u32)) {
                Entry::Occupied(x) => {
                    if txid == x.get() {
                        // This is a duplicate entry
                        continue;
                    } else {
                        return Err(Error::ConflictingTxLocator);
                    }
                }
                Entry::Vacant(entry) => {
                    entry.insert(*txid);
                }
            }
            for nf in nullifiers.iter() {
                self.nullifiers
                    .insert(block_height, *tx_index as u32, Nullifier::Sapling(*nf));
            }
        }
        Ok(())
    }

    #[cfg(feature = "orchard")]
    pub(crate) fn insert_orchard_nullifier_map(
        &mut self,
        block_height: BlockHeight,
        new_entries: &[(TxId, u16, Vec<orchard::note::Nullifier>)],
    ) -> Result<(), Error> {
        for (txid, tx_index, nullifiers) in new_entries {
            match self.tx_locator.entry((block_height, *tx_index as u32)) {
                Entry::Occupied(x) => {
                    if txid == x.get() {
                        // This is a duplicate entry
                        continue;
                    } else {
                        return Err(Error::ConflictingTxLocator);
                    }
                }
                Entry::Vacant(entry) => {
                    entry.insert(*txid);
                }
            }
            for nf in nullifiers.iter() {
                self.nullifiers
                    .insert(block_height, *tx_index as u32, Nullifier::Orchard(*nf));
            }
        }
        Ok(())
    }

    pub(crate) fn block_height_extrema(&self) -> Option<RangeInclusive<BlockHeight>> {
        let (min, max) = self.blocks.keys().fold((None, None), |(min, max), height| {
            (
                Some(min.map_or(height, |min| std::cmp::min(min, height))),
                Some(max.map_or(height, |max| std::cmp::max(max, height))),
            )
        });
        if let (Some(min), Some(max)) = (min, max) {
            Some(*min..=*max)
        } else {
            None
        }
    }

    pub(crate) fn sapling_tip_shard_end_height(&self) -> Option<BlockHeight> {
        self.sapling_tree_shard_end_heights.values().max().copied()
    }

    #[cfg(feature = "orchard")]
    pub(crate) fn orchard_tip_shard_end_height(&self) -> Option<BlockHeight> {
        self.orchard_tree_shard_end_heights.values().max().copied()
    }

    pub(crate) fn get_sapling_max_checkpointed_height(
        &self,
        chain_tip_height: BlockHeight,
        min_confirmations: NonZeroU32,
    ) -> Result<Option<BlockHeight>, Error> {
        let max_checkpoint_height =
            u32::from(chain_tip_height).saturating_sub(u32::from(min_confirmations) - 1);
        // scan backward and find the first checkpoint that matches a blockheight prior to max_checkpoint_height
        for height in max_checkpoint_height..0 {
            let height = BlockHeight::from_u32(height);
            if self.sapling_tree.store().get_checkpoint(&height)?.is_some() {
                return Ok(Some(height));
            }
        }
        Ok(None)
    }

    #[cfg(feature = "orchard")]
    pub(crate) fn get_orchard_max_checkpointed_height(
        &self,
        chain_tip_height: BlockHeight,
        min_confirmations: NonZeroU32,
    ) -> Result<Option<BlockHeight>, Error> {
        let max_checkpoint_height =
            u32::from(chain_tip_height).saturating_sub(u32::from(min_confirmations) - 1);
        // scan backward and find the first checkpoint that matches a blockheight prior to max_checkpoint_height
        for height in max_checkpoint_height..0 {
            let height = BlockHeight::from_u32(height);
            if self.sapling_tree.store().get_checkpoint(&height)?.is_some() {
                return Ok(Some(height));
            }
        }
        Ok(None)
    }
}
