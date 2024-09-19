use incrementalmerkletree::{Marking, Position, Retention};

use secrecy::SecretVec;
use shardtree::{error::ShardTreeError, store::ShardStore};

use std::{
    collections::{btree_map::Entry, BTreeMap, HashMap},
    ops::Range,
};

use zcash_primitives::{consensus::BlockHeight, transaction::TxId};
use zcash_protocol::{
    consensus::{self, NetworkUpgrade},
    ShieldedProtocol::{self, Sapling},
};

#[cfg(feature = "orchard")]
use zcash_client_backend::data_api::ORCHARD_SHARD_HEIGHT;
use zcash_client_backend::{
    address::UnifiedAddress,
    data_api::{
        chain::ChainState,
        scanning::{ScanPriority, ScanRange},
        AccountPurpose, AccountSource, TransactionStatus, WalletCommitmentTrees as _,
        SAPLING_SHARD_HEIGHT,
    },
    keys::{UnifiedAddressRequest, UnifiedFullViewingKey, UnifiedSpendingKey},
    wallet::{NoteId, Recipient, WalletTransparentOutput},
};

use zcash_client_backend::data_api::{
    AccountBirthday, DecryptedTransaction, ScannedBlock, SentTransaction, WalletRead, WalletWrite,
};

use crate::{
    error::Error, transparent::ReceivedTransparentOutput, PRUNING_DEPTH, VERIFY_LOOKAHEAD,
};
use crate::{MemoryWalletBlock, MemoryWalletDb, Nullifier, ReceivedNote};
use rayon::prelude::*;

use {secrecy::ExposeSecret, zip32::fingerprint::SeedFingerprint};

#[cfg(feature = "orchard")]
use zcash_protocol::ShieldedProtocol::Orchard;

impl<P: consensus::Parameters> WalletWrite for MemoryWalletDb<P> {
    type UtxoRef = u32;

    fn create_account(
        &mut self,
        seed: &SecretVec<u8>,
        birthday: &AccountBirthday,
    ) -> Result<(Self::AccountId, UnifiedSpendingKey), Self::Error> {
        if cfg!(not(test)) {
            unimplemented!(
                "Memwallet does not support adding accounts from seed phrases. 
    Instead derive the ufvk in the calling code and import it using `import_account_ufvk`"
            )
        } else {
            let seed_fingerprint = SeedFingerprint::from_seed(seed.expose_secret())
                .ok_or_else(|| Self::Error::InvalidSeedLength)?;
            let account_index = self
                .max_zip32_account_index(&seed_fingerprint)
                .unwrap()
                .map(|a| a.next().ok_or_else(|| Self::Error::AccountOutOfRange))
                .transpose()?
                .unwrap_or(zip32::AccountId::ZERO);

            let usk =
                UnifiedSpendingKey::from_seed(&self.params, seed.expose_secret(), account_index)?;
            let ufvk = usk.to_unified_full_viewing_key();

            let (id, _account) = self.add_account(
                AccountSource::Derived {
                    seed_fingerprint,
                    account_index,
                },
                ufvk,
                birthday.clone(),
                AccountPurpose::Spending,
            )?;

            Ok((id, usk))
        }
    }

    fn get_next_available_address(
        &mut self,
        account: Self::AccountId,
        request: UnifiedAddressRequest,
    ) -> Result<Option<UnifiedAddress>, Self::Error> {
        tracing::debug!("get_next_available_address");
        self.accounts
            .get_mut(account)
            .map(|account| account.next_available_address(request))
            .transpose()
            .map(|a| a.flatten())
    }

    fn update_chain_tip(&mut self, tip_height: BlockHeight) -> Result<(), Self::Error> {
        tracing::debug!("update_chain_tip");
        // If the caller provided a chain tip that is before Sapling activation, do nothing.
        let sapling_activation = match self.params.activation_height(NetworkUpgrade::Sapling) {
            Some(h) if h <= tip_height => h,
            _ => return Ok(()),
        };

        let max_scanned = self.block_height_extrema().map(|range| *range.end());
        let wallet_birthday = self.get_wallet_birthday()?;

        // If the chain tip is below the prior max scanned height, then the caller has caught
        // the chain in the middle of a reorg. Do nothing; the caller will continue using the
        // old scan ranges and either:
        // - encounter an error trying to fetch the blocks (and thus trigger the same handling
        //   logic as if this happened with the old linear scanning code); or
        // - encounter a discontinuity error in `scan_cached_blocks`, at which point they will
        //   call `WalletDb::truncate_to_height` as part of their reorg handling which will
        //   resolve the problem.
        //
        // We don't check the shard height, as normal usage would have the caller update the
        // shard state prior to this call, so it is possible and expected to be in a situation
        // where we should update the tip-related scan ranges but not the shard-related ones.
        match max_scanned {
            Some(h) if tip_height < h => return Ok(()),
            _ => (),
        };

        // `ScanRange` uses an exclusive upper bound.
        let chain_end = tip_height + 1;

        let sapling_shard_tip = self.sapling_tip_shard_end_height();
        // TODO: Handle orchard case as well. See zcash_client_sqlite scanning.rs update_chain_tip
        let min_shard_tip = sapling_shard_tip;

        // Create a scanning range for the fragment of the last shard leading up to new tip.
        // We set a lower bound at the wallet birthday (if known), because account creation
        // requires specifying a tree frontier that ensures we don't need tree information
        // prior to the birthday.
        let tip_shard_entry = min_shard_tip.filter(|h| h < &chain_end).map(|h| {
            let min_to_scan = wallet_birthday.filter(|b| b > &h).unwrap_or(h);
            ScanRange::from_parts(min_to_scan..chain_end, ScanPriority::ChainTip)
        });

        // Create scan ranges to either validate potentially invalid blocks at the wallet's
        // view of the chain tip, or connect the prior tip to the new tip.
        let tip_entry = max_scanned.map_or_else(
            || {
                // No blocks have been scanned, so we need to anchor the start of the new scan
                // range to something else.
                wallet_birthday.map_or_else(
                    // We don't have a wallet birthday, which means we have no accounts yet.
                    // We can therefore ignore all blocks up to the chain tip.
                    || ScanRange::from_parts(sapling_activation..chain_end, ScanPriority::Ignored),
                    // We have a wallet birthday, so mark all blocks between that and the
                    // chain tip as `Historic` (performing wallet recovery).
                    |wallet_birthday| {
                        ScanRange::from_parts(wallet_birthday..chain_end, ScanPriority::Historic)
                    },
                )
            },
            |max_scanned| {
                // The scan range starts at the block after the max scanned height. Since
                // `scan_cached_blocks` retrieves the metadata for the block being connected to
                // (if it exists), the connectivity of the scan range to the max scanned block
                // will always be checked if relevant.
                let min_unscanned = max_scanned + 1;

                // If we don't have shard metadata, this means we're doing linear scanning, so
                // create a scan range from the prior tip to the current tip with `Historic`
                // priority.
                if tip_shard_entry.is_none() {
                    ScanRange::from_parts(min_unscanned..chain_end, ScanPriority::Historic)
                } else {
                    // Determine the height to which we expect new blocks retrieved from the
                    // block source to be stable and not subject to being reorg'ed.
                    let stable_height = tip_height.saturating_sub(PRUNING_DEPTH);

                    // If the wallet's max scanned height is above the stable height,
                    // prioritize the range between it and the new tip as `ChainTip`.
                    if max_scanned > stable_height {
                        // We are in the steady-state case, where a wallet is close to the
                        // chain tip and just needs to catch up.
                        //
                        // This overlaps the `tip_shard_entry` range and so will be coalesced
                        // with it.
                        ScanRange::from_parts(min_unscanned..chain_end, ScanPriority::ChainTip)
                    } else {
                        // In this case, the max scanned height is considered stable relative
                        // to the chain tip. However, it may be stable or unstable relative to
                        // the prior chain tip, which we could determine by looking up the
                        // prior chain tip height from the scan queue. For simplicity we merge
                        // these two cases together, and proceed as though the max scanned
                        // block is unstable relative to the prior chain tip.
                        //
                        // To confirm its stability, prioritize the `VERIFY_LOOKAHEAD` blocks
                        // above the max scanned height as `Verify`:
                        //
                        // - We use `Verify` to ensure that a connectivity check is performed,
                        //   along with any required rewinds, before any `ChainTip` ranges
                        //   (from this or any prior `update_chain_tip` call) are scanned.
                        //
                        // - We prioritize `VERIFY_LOOKAHEAD` blocks because this is expected
                        //   to be 12.5 minutes, within which it is reasonable for a user to
                        //   have potentially received a transaction (if they opened their
                        //   wallet to provide an address to someone else, or spent their own
                        //   funds creating a change output), without necessarily having left
                        //   their wallet open long enough for the transaction to be mined and
                        //   the corresponding block to be scanned.
                        //
                        // - We limit the range to at most the stable region, to prevent any
                        //   `Verify` ranges from being susceptible to reorgs, and potentially
                        //   interfering with subsequent `Verify` ranges defined by future
                        //   calls to `update_chain_tip`. Any gap between `stable_height` and
                        //   `shard_start_height` will be filled by the scan range merging
                        //   logic with a `Historic` range.
                        //
                        // If `max_scanned == stable_height` then this is a zero-length range.
                        // In this case, any non-empty `(stable_height+1)..shard_start_height`
                        // will be marked `Historic`, minimising the prioritised blocks at the
                        // chain tip and allowing for other ranges (for example, `FoundNote`)
                        // to take priority.
                        ScanRange::from_parts(
                            min_unscanned
                                ..std::cmp::min(
                                    stable_height + 1,
                                    min_unscanned + VERIFY_LOOKAHEAD,
                                ),
                            ScanPriority::Verify,
                        )
                    }
                }
            },
        );
        if let Some(entry) = &tip_shard_entry {
            tracing::debug!("{} will update latest shard", entry);
        }
        tracing::debug!("{} will connect prior scanned state to new tip", tip_entry);

        let query_range = match tip_shard_entry.as_ref() {
            Some(se) => Range {
                start: std::cmp::min(se.block_range().start, tip_entry.block_range().start),
                end: std::cmp::max(se.block_range().end, tip_entry.block_range().end),
            },
            None => tip_entry.block_range().clone(),
        };

        self.scan_queue.replace_queue_entries(
            &query_range,
            tip_shard_entry.into_iter().chain(Some(tip_entry)),
            false,
        )?;
        Ok(())
    }

    /// Adds a sequence of blocks to the data store.
    ///
    /// Assumes blocks will be here in order.
    fn put_blocks(
        &mut self,
        from_state: &ChainState,
        blocks: Vec<ScannedBlock<Self::AccountId>>,
    ) -> Result<(), Self::Error> {
        tracing::debug!("put_blocks");
        // TODO:
        // - Make sure blocks are coming in order.
        // - Make sure the first block in the sequence is tip + 1?
        // - Add a check to make sure the blocks are not already in the data store.
        // let _start_height = blocks.first().map(|b| b.height());
        let mut last_scanned_height = None;
        struct BlockPositions {
            height: BlockHeight,
            sapling_start_position: Position,
            #[cfg(feature = "orchard")]
            orchard_start_position: Position,
        }
        let start_positions = blocks.first().map(|block| BlockPositions {
            height: block.height(),
            sapling_start_position: Position::from(
                u64::from(block.sapling().final_tree_size())
                    - u64::try_from(block.sapling().commitments().len()).unwrap(),
            ),
            #[cfg(feature = "orchard")]
            orchard_start_position: Position::from(
                u64::from(block.orchard().final_tree_size())
                    - u64::try_from(block.orchard().commitments().len()).unwrap(),
            ),
        });

        let mut sapling_commitments = vec![];
        #[cfg(feature = "orchard")]
        let mut orchard_commitments = vec![];
        let mut note_positions = vec![];
        for block in blocks.into_iter() {
            let mut transactions = HashMap::new();
            let mut memos = HashMap::new();
            if last_scanned_height
                .iter()
                .any(|prev| block.height() != *prev + 1)
            {
                return Err(Error::NonSequentialBlocks);
            }

            for transaction in block.transactions().iter() {
                let txid = transaction.txid();

                // Mark the Sapling nullifiers of the spent notes as spent in the `sapling_spends` map.
                for spend in transaction.sapling_spends() {
                    self.mark_sapling_note_spent(*spend.nf(), txid)?;
                }

                // Mark the Orchard nullifiers of the spent notes as spent in the `orchard_spends` map.
                #[cfg(feature = "orchard")]
                for spend in transaction.orchard_spends() {
                    self.mark_orchard_note_spent(*spend.nf(), txid)?;
                }

                for output in transaction.sapling_outputs() {
                    // Insert the memo into the `memos` map.
                    let note_id = NoteId::new(
                        txid,
                        Sapling,
                        u16::try_from(output.index())
                            .expect("output indices are representable as u16"),
                    );
                    if let Ok(Some(memo)) = self.get_memo(note_id) {
                        memos.insert(note_id, memo.encode());
                    }
                    // Check whether this note was spent in a later block range that
                    // we previously scanned.
                    let spent_in = output
                        .nf()
                        .and_then(|nf| self.nullifiers.get(&Nullifier::Sapling(*nf)))
                        .and_then(|(height, tx_idx)| self.tx_locator.get(*height, *tx_idx))
                        .copied();

                    self.insert_received_sapling_note(note_id, output, spent_in);
                }

                #[cfg(feature = "orchard")]
                for output in transaction.orchard_outputs().iter() {
                    // Insert the memo into the `memos` map.
                    let note_id = NoteId::new(
                        txid,
                        Orchard,
                        u16::try_from(output.index())
                            .expect("output indices are representable as u16"),
                    );
                    if let Ok(Some(memo)) = self.get_memo(note_id) {
                        memos.insert(note_id, memo.encode());
                    }
                    // Check whether this note was spent in a later block range that
                    // we previously scanned.
                    let spent_in = output
                        .nf()
                        .and_then(|nf| self.nullifiers.get(&Nullifier::Orchard(*nf)))
                        .and_then(|(height, tx_idx)| self.tx_locator.get(*height, *tx_idx))
                        .copied();

                    self.insert_received_orchard_note(note_id, output, spent_in)
                }

                transactions.insert(txid, transaction.clone());
            }

            // Insert the new nullifiers from this block into the nullifier map
            self.insert_sapling_nullifier_map(block.height(), block.sapling().nullifier_map())?;
            #[cfg(feature = "orchard")]
            self.insert_orchard_nullifier_map(block.height(), block.orchard().nullifier_map())?;
            note_positions.extend(block.transactions().iter().flat_map(|wtx| {
                let iter = wtx.sapling_outputs().iter().map(|out| {
                    (
                        ShieldedProtocol::Sapling,
                        out.note_commitment_tree_position(),
                    )
                });
                #[cfg(feature = "orchard")]
                let iter = iter.chain(wtx.orchard_outputs().iter().map(|out| {
                    (
                        ShieldedProtocol::Orchard,
                        out.note_commitment_tree_position(),
                    )
                }));

                iter
            }));

            let memory_block = MemoryWalletBlock {
                height: block.height(),
                hash: block.block_hash(),
                block_time: block.block_time(),
                _transactions: transactions.keys().cloned().collect(),
                _memos: memos,
                sapling_commitment_tree_size: Some(block.sapling().final_tree_size()),
                sapling_output_count: Some(block.sapling().commitments().len().try_into().unwrap()),
                #[cfg(feature = "orchard")]
                orchard_commitment_tree_size: Some(block.orchard().final_tree_size()),
                #[cfg(feature = "orchard")]
                orchard_action_count: Some(block.orchard().commitments().len().try_into().unwrap()),
            };

            // Insert transaction metadata into the transaction table
            transactions
                .into_iter()
                .for_each(|(_id, tx)| self.tx_table.put_tx_meta(tx, block.height()));

            // Insert the block into the block map
            self.blocks.insert(block.height(), memory_block);
            last_scanned_height = Some(block.height());

            let block_commitments = block.into_commitments();
            sapling_commitments.extend(block_commitments.sapling.into_iter().map(Some));
            #[cfg(feature = "orchard")]
            orchard_commitments.extend(block_commitments.orchard.into_iter().map(Some));
        }

        // TODO: Prune the nullifier map of entries we no longer need.

        if let Some((start_positions, last_scanned_height)) =
            start_positions.zip(last_scanned_height)
        {
            // Create subtrees from the note commitments in parallel.
            const CHUNK_SIZE: usize = 1024;
            let sapling_subtrees = sapling_commitments
                .par_chunks_mut(CHUNK_SIZE)
                .enumerate()
                .filter_map(|(i, chunk)| {
                    let start = start_positions.sapling_start_position + (i * CHUNK_SIZE) as u64;
                    let end = start + chunk.len() as u64;

                    shardtree::LocatedTree::from_iter(
                        start..end,
                        SAPLING_SHARD_HEIGHT.into(),
                        chunk.iter_mut().map(|n| n.take().expect("always Some")),
                    )
                })
                .map(|res| (res.subtree, res.checkpoints))
                .collect::<Vec<_>>();

            #[cfg(feature = "orchard")]
            let orchard_subtrees = orchard_commitments
                .par_chunks_mut(CHUNK_SIZE)
                .enumerate()
                .filter_map(|(i, chunk)| {
                    let start = start_positions.orchard_start_position + (i * CHUNK_SIZE) as u64;
                    let end = start + chunk.len() as u64;

                    shardtree::LocatedTree::from_iter(
                        start..end,
                        ORCHARD_SHARD_HEIGHT.into(),
                        chunk.iter_mut().map(|n| n.take().expect("always Some")),
                    )
                })
                .map(|res| (res.subtree, res.checkpoints))
                .collect::<Vec<_>>();

            // Collect the complete set of Sapling checkpoints
            #[cfg(feature = "orchard")]
            let sapling_checkpoint_positions: BTreeMap<BlockHeight, Position> = sapling_subtrees
                .iter()
                .flat_map(|(_, checkpoints)| checkpoints.iter())
                .map(|(k, v)| (*k, *v))
                .collect();

            #[cfg(feature = "orchard")]
            let orchard_checkpoint_positions: BTreeMap<BlockHeight, Position> = orchard_subtrees
                .iter()
                .flat_map(|(_, checkpoints)| checkpoints.iter())
                .map(|(k, v)| (*k, *v))
                .collect();

            #[cfg(feature = "orchard")]
            let (missing_sapling_checkpoints, missing_orchard_checkpoints) = (
                ensure_checkpoints(
                    orchard_checkpoint_positions.keys(),
                    &sapling_checkpoint_positions,
                    from_state.final_sapling_tree(),
                ),
                ensure_checkpoints(
                    sapling_checkpoint_positions.keys(),
                    &orchard_checkpoint_positions,
                    from_state.final_orchard_tree(),
                ),
            );

            // Update the Sapling note commitment tree with all newly read note commitments
            {
                let mut sapling_subtrees_iter = sapling_subtrees.into_iter();
                self.with_sapling_tree_mut::<_, _, Self::Error>(|sapling_tree| {
                    sapling_tree.insert_frontier(
                        from_state.final_sapling_tree().clone(),
                        Retention::Checkpoint {
                            id: from_state.block_height(),
                            marking: Marking::Reference,
                        },
                    )?;

                    for (tree, checkpoints) in &mut sapling_subtrees_iter {
                        sapling_tree.insert_tree(tree, checkpoints)?;
                    }

                    // Ensure we have a Sapling checkpoint for each checkpointed Orchard block height.
                    // We skip all checkpoints below the minimum retained checkpoint in the
                    // Sapling tree, because branches below this height may be pruned.
                    #[cfg(feature = "orchard")]
                    {
                        let min_checkpoint_height = sapling_tree
                            .store()
                            .min_checkpoint_id()
                            .map_err(ShardTreeError::Storage)?
                            .expect("At least one checkpoint was inserted (by insert_frontier)");

                        for (height, checkpoint) in &missing_sapling_checkpoints {
                            if *height > min_checkpoint_height {
                                sapling_tree
                                    .store_mut()
                                    .add_checkpoint(*height, checkpoint.clone())
                                    .map_err(ShardTreeError::Storage)?;
                            }
                        }
                    }

                    Ok(())
                })?;
            }

            // Update the Orchard note commitment tree with all newly read note commitments
            #[cfg(feature = "orchard")]
            {
                let mut orchard_subtrees = orchard_subtrees.into_iter();
                self.with_orchard_tree_mut::<_, _, Self::Error>(|orchard_tree| {
                    orchard_tree.insert_frontier(
                        from_state.final_orchard_tree().clone(),
                        Retention::Checkpoint {
                            id: from_state.block_height(),
                            marking: Marking::Reference,
                        },
                    )?;

                    for (tree, checkpoints) in &mut orchard_subtrees {
                        orchard_tree.insert_tree(tree, checkpoints)?;
                    }

                    // Ensure we have an Orchard checkpoint for each checkpointed Sapling block height.
                    // We skip all checkpoints below the minimum retained checkpoint in the
                    // Orchard tree, because branches below this height may be pruned.
                    {
                        let min_checkpoint_height = orchard_tree
                            .store()
                            .min_checkpoint_id()
                            .map_err(ShardTreeError::Storage)?
                            .expect("At least one checkpoint was inserted (by insert_frontier)");

                        for (height, checkpoint) in &missing_orchard_checkpoints {
                            if *height > min_checkpoint_height {
                                orchard_tree
                                    .store_mut()
                                    .add_checkpoint(*height, checkpoint.clone())
                                    .map_err(ShardTreeError::Storage)?;
                            }
                        }
                    }
                    Ok(())
                })?;
            }

            self.scan_complete(
                Range {
                    start: start_positions.height,
                    end: last_scanned_height + 1,
                },
                &note_positions,
            )?;
        }

        // We can do some pruning of the tx_locator_map here

        Ok(())
    }

    /// Adds a transparent UTXO received by the wallet to the data store.
    fn put_received_transparent_utxo(
        &mut self,
        output: &WalletTransparentOutput,
    ) -> Result<Self::UtxoRef, Self::Error> {
        tracing::debug!("put_received_transparent_utxo");
        #[cfg(feature = "transparent-inputs")]
        {
            let address = output.recipient_address();
            if let Some(receiving_account) = self.find_account_for_transparent_address(address)? {
                // get the block height of the block that mined the output only if we have it in the block table
                // otherwise return None
                let block = output
                    .mined_height()
                    .map(|h| self.blocks.get(&h).map(|b| b.height))
                    .flatten();
                let txid = TxId::from_bytes(output.outpoint().hash().to_vec().try_into().unwrap());

                // insert a new tx into the transactions table for the one that spent this output. If there is already one then do an update
                self.tx_table
                    .put_tx_partial(&txid, &block, output.mined_height());

                // look for a spent_height for this output by querying transparent_received_output_spends.
                // If there isn't one then return None (this is an unspent output)
                // otherwise return the height found by joining on the tx table
                let spent_height = self
                    .transparent_received_output_spends
                    .get(&output.outpoint())
                    .map(|txid| {
                        self.tx_table
                            .tx_status(txid)
                            .map(|status| match status {
                                TransactionStatus::Mined(height) => Some(height),
                                _ => None,
                            })
                            .flatten()
                    })
                    .flatten();

                // The max observed unspent height is either the spending transaction's mined height - 1, or
                // the current chain tip height (assuming we know it is unspent)
                let max_observed_unspent = match spent_height {
                    Some(h) => Some(h - 1),
                    None => self.chain_height()?,
                }.unwrap_or(BlockHeight::from(0));

                // insert into transparent_received_outputs table. Update if it exists
                match self
                    .transparent_received_outputs
                    .entry(output.outpoint().clone())
                {
                    Entry::Occupied(mut entry) => {
                        entry.get_mut().transaction_id = txid;
                        entry.get_mut().address = *address;
                        entry.get_mut().account_id = receiving_account;
                        entry.get_mut().txout = output.txout().clone();
                    }
                    Entry::Vacant(entry) => {
                        entry.insert(ReceivedTransparentOutput::new(
                            txid,
                            receiving_account,
                            *address,
                            output.txout().clone(),
                            max_observed_unspent,
                        ));
                    }
                }

                // look in transparent_spend_map for a record of the output already having been spent, then mark it as spent using the
                // stored reference to the spending transaction.
                if self.transparent_spend_map.contains(&txid, output.outpoint()) {
                    self.mark_transparent_output_spent(&txid, output.outpoint())?;
                }

                todo!()
            } else {
                // The UTXO was not for any of our transparent addresses.
                Err(Error::AddressNotRecognized(*address))
            }
        }
        #[cfg(not(feature = "transparent-inputs"))]
        panic!(
            "The wallet must be compiled with the transparent-inputs feature to use this method."
        );
    }

    fn store_decrypted_tx(
        &mut self,
        d_tx: DecryptedTransaction<Self::AccountId>,
    ) -> Result<(), Self::Error> {
        tracing::debug!("store_decrypted_tx");
        self.tx_table.put_tx_data(d_tx.tx(), None, None);
        if let Some(height) = d_tx.mined_height() {
            self.set_transaction_status(d_tx.tx().txid(), TransactionStatus::Mined(height))?
        }
        Ok(())
    }

    /// Truncates the database to the given height.
    ///
    /// If the requested height is greater than or equal to the height of the last scanned
    /// block, this function does nothing.
    ///
    /// This should only be executed inside a transactional context.
    fn truncate_to_height(&mut self, _block_height: BlockHeight) -> Result<(), Self::Error> {
        todo!()
    }

    fn import_account_hd(
        &mut self,
        _seed: &SecretVec<u8>,
        _account_index: zip32::AccountId,
        _birthday: &AccountBirthday,
    ) -> Result<(Self::Account, UnifiedSpendingKey), Self::Error> {
        unimplemented!(
            "Memwallet does not support adding accounts from seed phrases. 
Instead derive the ufvk in the calling code and import it using `import_account_ufvk`"
        )
    }

    fn import_account_ufvk(
        &mut self,
        unified_key: &UnifiedFullViewingKey,
        birthday: &AccountBirthday,
        purpose: AccountPurpose,
    ) -> Result<Self::Account, Self::Error> {
        tracing::debug!("import_account_ufvk");
        let (_id, account) = self.add_account(
            AccountSource::Imported { purpose },
            unified_key.to_owned(),
            birthday.clone(),
            purpose,
        )?;
        Ok(account)
    }

    fn store_transactions_to_be_sent(
        &mut self,
        transactions: &[SentTransaction<Self::AccountId>],
    ) -> Result<(), Self::Error> {
        tracing::debug!("store_transactions_to_be_sent");
        for sent_tx in transactions {
            self.tx_table.put_tx_data(
                sent_tx.tx(),
                Some(sent_tx.fee_amount()),
                Some(sent_tx.target_height()),
            );
            // Mark sapling notes as spent
            if let Some(bundle) = sent_tx.tx().sapling_bundle() {
                for spend in bundle.shielded_spends() {
                    self.mark_sapling_note_spent(*spend.nullifier(), sent_tx.tx().txid())?;
                }
            }
            // Mark orchard notes as spent
            if let Some(bundle) = sent_tx.tx().orchard_bundle() {
                #[cfg(feature = "orchard")]
                {
                    for action in bundle.actions() {
                        match self.mark_orchard_note_spent(*action.nullifier(), sent_tx.tx().txid())
                        {
                            Ok(()) => {}
                            Err(Error::NoteNotFound) => {
                                // This is expected as some of the actions will be new outputs we don't have notes for
                                // The ones we do recognize will be marked as spent
                            }
                            Err(e) => return Err(e),
                        }
                    }
                }

                #[cfg(not(feature = "orchard"))]
                panic!("Sent a transaction with Orchard Actions without `orchard` enabled?");
            }
            // Mark transparent UTXOs as spent
            #[cfg(feature = "transparent-inputs")]
            for _utxo_outpoint in sent_tx.utxos_spent() {
                todo!()
            }

            for output in sent_tx.outputs() {
                self.sent_notes.insert_sent_output(sent_tx, output);

                match output.recipient() {
                    Recipient::InternalAccount { .. } => {
                        self.received_notes.insert_received_note(
                            ReceivedNote::from_sent_tx_output(sent_tx.tx().txid(), output)?,
                        );
                    }
                    Recipient::EphemeralTransparent {
                        receiving_account: _,
                        ephemeral_address: _,
                        outpoint_metadata: _,
                    } => {
                        // mark ephemeral address as used
                    }
                    Recipient::External(_, _) => {}
                }
            }
            // in sqlite they que
        }
        Ok(())
    }

    fn set_transaction_status(
        &mut self,
        txid: TxId,
        status: TransactionStatus,
    ) -> Result<(), Self::Error> {
        tracing::debug!("set_transaction_status");
        self.tx_table.set_transaction_status(&txid, status)
    }
}

#[cfg(feature = "orchard")]
use {incrementalmerkletree::frontier::Frontier, shardtree::store::Checkpoint};

#[cfg(feature = "orchard")]
fn ensure_checkpoints<'a, H, I: Iterator<Item = &'a BlockHeight>, const DEPTH: u8>(
    // An iterator of checkpoints heights for which we wish to ensure that
    // checkpoints exists.
    ensure_heights: I,
    // The map of checkpoint positions from which we will draw note commitment tree
    // position information for the newly created checkpoints.
    existing_checkpoint_positions: &BTreeMap<BlockHeight, Position>,
    // The frontier whose position will be used for an inserted checkpoint when
    // there is no preceding checkpoint in existing_checkpoint_positions.
    state_final_tree: &Frontier<H, DEPTH>,
) -> Vec<(BlockHeight, Checkpoint)> {
    ensure_heights
        .flat_map(|ensure_height| {
            existing_checkpoint_positions
                .range::<BlockHeight, _>(..=*ensure_height)
                .last()
                .map_or_else(
                    || {
                        Some((
                            *ensure_height,
                            state_final_tree
                                .value()
                                .map_or_else(Checkpoint::tree_empty, |t| {
                                    Checkpoint::at_position(t.position())
                                }),
                        ))
                    },
                    |(existing_checkpoint_height, position)| {
                        if *existing_checkpoint_height < *ensure_height {
                            Some((*ensure_height, Checkpoint::at_position(*position)))
                        } else {
                            // The checkpoint already exists, so we don't need to
                            // do anything.
                            None
                        }
                    },
                )
                .into_iter()
        })
        .collect::<Vec<_>>()
}
