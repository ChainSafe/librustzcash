use std::{
    collections::{btree_map::Entry, BTreeMap, BTreeSet},
    ops::{Deref, DerefMut},
};

use serde::{Deserialize, Serialize};
use serde_with::{serde_as, FromInto};
use zcash_client_backend::wallet::WalletTransparentOutput;
use zcash_primitives::{
    legacy::TransparentAddress,
    transaction::{
        components::{OutPoint, TxOut},
        TxId,
    },
};
use zcash_protocol::consensus::BlockHeight;

use super::AccountId;
use crate::{ByteArray, OutPointDef, TransparentAddressDef, TxOutDef};

/// Stores the transparent outputs received by the wallet.
#[serde_as]
#[derive(Default, Serialize, Deserialize)]
pub struct TransparentReceivedOutputs(
    #[serde_as(as = "BTreeMap<OutPointDef, _>")] BTreeMap<OutPoint, ReceivedTransparentOutput>,
);

impl TransparentReceivedOutputs {
    pub fn new() -> Self {
        Self(BTreeMap::new())
    }

    pub fn get(&self, outpoint: &OutPoint) -> Option<&ReceivedTransparentOutput> {
        self.0.get(outpoint)
    }
}

impl Deref for TransparentReceivedOutputs {
    type Target = BTreeMap<OutPoint, ReceivedTransparentOutput>;

    fn deref(&self) -> &Self::Target {
        &self.0
    }
}

impl DerefMut for TransparentReceivedOutputs {
    fn deref_mut(&mut self) -> &mut Self::Target {
        &mut self.0
    }
}

/// A junction table between received transparent outputs and the transactions that spend them.
#[serde_as]
#[derive(Default, Serialize, Deserialize)]
pub struct TransparentReceivedOutputSpends(
    #[serde_as(as = "BTreeMap<OutPointDef, ByteArray<32>>")] BTreeMap<OutPoint, TxId>,
);

impl TransparentReceivedOutputSpends {
    pub fn new() -> Self {
        Self(BTreeMap::new())
    }

    pub fn get(&self, outpoint: &OutPoint) -> Option<&TxId> {
        self.0.get(outpoint)
    }

    pub fn entry(&mut self, outpoint: OutPoint) -> Entry<'_, OutPoint, TxId> {
        self.0.entry(outpoint)
    }

    pub fn insert(&mut self, outpoint: OutPoint, txid: TxId) {
        self.0.insert(outpoint, txid);
    }
}

impl Deref for TransparentReceivedOutputSpends {
    type Target = BTreeMap<OutPoint, TxId>;

    fn deref(&self) -> &Self::Target {
        &self.0
    }
}

// transparent_received_outputs
#[serde_as]
#[derive(Serialize, Deserialize)]
pub struct ReceivedTransparentOutput {
    // Reference to the transaction in which this TXO was created
    #[serde_as(as = "ByteArray<32>")]
    pub(crate) transaction_id: TxId,
    // The account that controls spend authority for this TXO
    pub(crate) account_id: AccountId,
    // The address to which this TXO was sent
    #[serde_as(as = "TransparentAddressDef")]
    pub(crate) address: TransparentAddress,
    // script, value_zat
    #[serde_as(as = "TxOutDef")]
    pub(crate) txout: TxOut,
    /// The maximum block height at which this TXO was either
    /// observed to be a member of the UTXO set at the start of the block, or observed
    /// to be an output of a transaction mined in the block. This is intended to be used to
    /// determine when the TXO is no longer a part of the UTXO set, in the case that the
    /// transaction that spends it is not detected by the wallet.
    #[serde_as(as = "Option<FromInto<u32>>")]
    pub(crate) max_observed_unspent_height: Option<BlockHeight>,
}

impl ReceivedTransparentOutput {
    pub fn new(
        transaction_id: TxId,
        account_id: AccountId,
        address: TransparentAddress,
        txout: TxOut,
        max_observed_unspent_height: BlockHeight,
    ) -> Self {
        Self {
            transaction_id,
            account_id,
            address,
            txout,
            max_observed_unspent_height: Some(max_observed_unspent_height),
        }
    }

    pub fn to_wallet_transparent_output(
        &self,
        outpoint: &OutPoint,
        mined_height: Option<BlockHeight>,
    ) -> Option<WalletTransparentOutput> {
        WalletTransparentOutput::from_parts(outpoint.clone(), self.txout.clone(), mined_height)
    }
}

/// A cache of the relationship between a transaction and the prevout data of its
/// transparent inputs.
///
/// Output may be attempted to be spent in multiple transactions, even though only one will ever be mined
/// which is why can cannot just rely on TransparentReceivedOutputSpends or implement this as as map
#[serde_as]
#[derive(Default, Serialize, Deserialize)]
pub struct TransparentSpendCache(
    #[serde_as(as = "BTreeSet<(ByteArray<32>, OutPointDef)>")] BTreeSet<(TxId, OutPoint)>,
);

impl TransparentSpendCache {
    pub fn new() -> Self {
        Self(BTreeSet::new())
    }

    /// Get all the outpoints for a given transaction ID.
    pub fn contains(&self, txid: &TxId, outpoint: &OutPoint) -> bool {
        self.0.contains(&(*txid, outpoint.clone()))
    }

    pub fn insert(&mut self, txid: TxId, outpoint: OutPoint) {
        self.0.insert((txid, outpoint));
    }
}

impl Deref for TransparentSpendCache {
    type Target = BTreeSet<(TxId, OutPoint)>;

    fn deref(&self) -> &Self::Target {
        &self.0
    }
}
