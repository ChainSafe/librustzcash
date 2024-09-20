use std::{collections::{btree_map::Entry, BTreeMap}, ops::Deref};

use serde::{Deserialize, Serialize};
use zcash_primitives::{
    consensus::BlockHeight,
    transaction::{Transaction, TxId},
};
use zcash_protocol::value::Zatoshis;

use zcash_client_backend::{data_api::TransactionStatus, wallet::WalletTx};

use crate::AccountId;

use crate::error::Error;
use crate::types::serialization::*;
use serde_with::serde_as;
use serde_with::{FromInto, TryFromInto};
/// Maps a block height and transaction index to a transaction ID.
#[serde_as]
#[derive(Serialize, Deserialize)]
pub(crate) struct TxLocatorMap(
    #[serde_as(as = "BTreeMap<(FromInto<u32>, _), ByteArray<32>>")]
    BTreeMap<(BlockHeight, u32), TxId>,
);

/// A table of received notes. Corresponds to sapling_received_notes and orchard_received_notes tables.
#[serde_as]
#[derive(Serialize, Deserialize)]
pub(crate) struct TransactionEntry {
    // created: String,
    /// mined_height is rolled into into a txn status
    #[serde(with = "TransactionStatusDef")]
    tx_status: TransactionStatus,
    #[serde_as(as = "Option<FromInto<u32>>")]
    block: Option<BlockHeight>,
    tx_index: Option<u32>,
    #[serde_as(as = "Option<FromInto<u32>>")]
    expiry_height: Option<BlockHeight>,
    raw: Option<Vec<u8>>,
    #[serde_as(as = "Option<TryFromInto<u64>>")]
    fee: Option<Zatoshis>,
    /// - `target_height`: stores the target height for which the transaction was constructed, if
    ///   known. This will ordinarily be null for transactions discovered via chain scanning; it
    ///   will only be set for transactions created using this wallet specifically, and not any
    ///   other wallet that uses the same seed (including previous installations of the same
    ///   wallet application.)
    #[serde_as(as = "Option<FromInto<u32>>")]
    _target_height: Option<BlockHeight>,
}
impl TransactionEntry {
    pub fn new_from_tx_meta(tx_meta: WalletTx<AccountId>, height: BlockHeight) -> Self {
        Self {
            tx_status: TransactionStatus::Mined(height),
            tx_index: Some(tx_meta.block_index() as u32),
            block: Some(height),
            expiry_height: None,
            raw: None,
            fee: None,
            _target_height: None,
        }
    }
    pub(crate) fn expiry_height(&self) -> Option<BlockHeight> {
        self.expiry_height
    }
    pub(crate) fn status(&self) -> TransactionStatus {
        self.tx_status
    }

    pub(crate) fn mined_height(&self) -> Option<BlockHeight> {
        match self.tx_status {
            TransactionStatus::Mined(height) => Some(height),
            _ => None,
        }
    }

    pub(crate) fn fee(&self) -> Option<Zatoshis> {
        self.fee
    }

    pub(crate) fn raw(&self) -> Option<&[u8]> {
        self.raw.as_ref().map(|v| v.as_slice())
    }
}
#[serde_as]
#[derive(Serialize, Deserialize)]
pub(crate) struct TransactionTable(
    #[serde_as(as = "BTreeMap<ByteArray<32>, _>")] BTreeMap<TxId, TransactionEntry>,
);

impl TransactionTable {
    pub(crate) fn new() -> Self {
        Self(BTreeMap::new())
    }
    /// Returns transaction status for a given transaction ID. None if the transaction is not known.
    pub(crate) fn tx_status(&self, txid: &TxId) -> Option<TransactionStatus> {
        self.0.get(txid).map(|entry| entry.tx_status)
    }
    pub(crate) fn expiry_height(&self, txid: &TxId) -> Option<BlockHeight> {
        self.0.get(txid).and_then(|entry| entry.expiry_height)
    }
    pub(crate) fn _get_transaction(&self, txid: TxId) -> Option<&TransactionEntry> {
        self.0.get(&txid)
    }

    /// Inserts information about a MINED transaction that was observed to
    /// contain a note related to this wallet
    pub(crate) fn put_tx_meta(&mut self, tx_meta: WalletTx<AccountId>, height: BlockHeight) {
        match self.0.entry(tx_meta.txid()) {
            Entry::Occupied(mut entry) => {
                entry.get_mut().tx_index = Some(tx_meta.block_index() as u32);
                entry.get_mut().tx_status = TransactionStatus::Mined(height);
            }
            Entry::Vacant(entry) => {
                entry.insert(TransactionEntry::new_from_tx_meta(tx_meta, height));
            }
        }
    }

    #[cfg(feature = "transparent-inputs")]
    /// Insert partial transaction data ontained from a received transparent output
    /// Will update an existing transaction if it already exists with new date (e.g. will replace Nones with newer Some value)
    pub(crate) fn put_tx_partial(
        &mut self,
        txid: &TxId,
        block: &Option<BlockHeight>,
        mined_height: Option<BlockHeight>,
    ) {
        match self.0.entry(*txid) {
            Entry::Occupied(mut entry) => {
                match entry.get().tx_status {
                    TransactionStatus::Mined(_) => {
                        // If the transaction is already mined, we don't need to update it
                        return;
                    }
                    _ => {
                        // If there was no info about the tx being mined we can update it if we have it
                        entry.get_mut().tx_status = mined_height
                            .map(|h| TransactionStatus::Mined(h))
                            .unwrap_or(TransactionStatus::NotInMainChain);
                    }
                }
                // replace the block if it's not already set
                entry.get_mut().block = (*block).or(entry.get().block);
            }
            Entry::Vacant(entry) => {
                entry.insert(TransactionEntry {
                    tx_status: mined_height
                        .map(|h| TransactionStatus::Mined(h))
                        .unwrap_or(TransactionStatus::NotInMainChain),
                    block: *block,
                    tx_index: None,
                    expiry_height: None,
                    raw: None,
                    fee: None,
                    _target_height: None,
                });
            }
        }
    }

    /// Inserts full transaction data
    pub(crate) fn put_tx_data(
        &mut self,
        tx: &Transaction,
        fee: Option<Zatoshis>,
        target_height: Option<BlockHeight>,
    ) {
        match self.0.entry(tx.txid()) {
            Entry::Occupied(mut entry) => {
                entry.get_mut().fee = fee;
                entry.get_mut().expiry_height = Some(tx.expiry_height());

                let mut raw = Vec::new();
                tx.write(&mut raw).unwrap();
                entry.get_mut().raw = Some(raw);
            }
            Entry::Vacant(entry) => {
                let mut raw = Vec::new();
                tx.write(&mut raw).unwrap();
                entry.insert(TransactionEntry {
                    tx_status: TransactionStatus::NotInMainChain,
                    tx_index: None,
                    block: None,
                    expiry_height: Some(tx.expiry_height()),
                    raw: Some(raw),
                    fee,
                    _target_height: target_height,
                });
            }
        }
    }
    pub(crate) fn set_transaction_status(
        &mut self,
        txid: &TxId,
        status: TransactionStatus,
    ) -> Result<(), Error> {
        if let Some(entry) = self.0.get_mut(txid) {
            entry.tx_status = status;
            Ok(())
        } else {
            Err(Error::TransactionNotFound(*txid))
        }
    }
    pub(crate) fn get_tx_raw(&self, txid: &TxId) -> Option<&[u8]> {
        self.0
            .get(txid)
            .map(|entry| entry.raw.as_ref().map(|v| v.as_slice()))
            .flatten()
    }
}

impl TransactionTable {
    pub(crate) fn get(&self, txid: &TxId) -> Option<&TransactionEntry> {
        self.0.get(txid)
    }

    pub(crate) fn _get_mut(&mut self, txid: &TxId) -> Option<&mut TransactionEntry> {
        self.0.get_mut(txid)
    }

    pub(crate) fn _remove(&mut self, txid: &TxId) -> Option<TransactionEntry> {
        self.0.remove(txid)
    }
}

// impl IntoIterator for TransactionTable {
//     type Item = (TxId, TransactionEntry);
//     type IntoIter = std::collections::btree_map::IntoIter<TxId, TransactionEntry>;

//     fn into_iter(self) -> Self::IntoIter {
//         self.0.into_iter()
//     }
// }

impl Deref for TransactionTable {
    type Target = BTreeMap<TxId, TransactionEntry>;

    fn deref(&self) -> &Self::Target {
        &self.0
    }
}

impl TxLocatorMap {
    pub(crate) fn new() -> Self {
        Self(BTreeMap::new())
    }
    pub(crate) fn _insert(&mut self, height: BlockHeight, index: u32, txid: TxId) {
        self.0.insert((height, index), txid);
    }

    pub(crate) fn get(&self, height: BlockHeight, index: u32) -> Option<&TxId> {
        self.0.get(&(height, index))
    }
    pub(crate) fn entry(&mut self, k: (BlockHeight, u32)) -> Entry<(BlockHeight, u32), TxId> {
        self.0.entry(k)
    }
}
