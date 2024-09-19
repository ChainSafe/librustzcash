use std::{collections::BTreeMap, ops::Deref};

use zcash_primitives::{
    legacy::TransparentAddress,
    transaction::{
        components::{OutPoint, TxOut},
        TxId,
    },
};

use super::AccountId;

/// Stores the transparent outputs received by the wallet.
#[derive(Default)]
pub struct TransparentReceivedOutputs(BTreeMap<OutPoint, ReceivedTransparentOutput>);

impl TransparentReceivedOutputs {
    pub fn new() -> Self {
        Self(BTreeMap::new())
    }
}

impl IntoIterator for TransparentReceivedOutputs {
    type Item = (OutPoint, ReceivedTransparentOutput);
    type IntoIter = <BTreeMap<OutPoint, ReceivedTransparentOutput> as IntoIterator>::IntoIter;

    fn into_iter(self) -> Self::IntoIter {
        self.0.into_iter()
    }
}

impl Deref for TransparentReceivedOutputs {
    type Target = BTreeMap<OutPoint, ReceivedTransparentOutput>;

    fn deref(&self) -> &Self::Target {
        &self.0
    }
}

/// A junction table between received transparent outputs and the transactions that spend them.
#[derive(Default)]
pub struct TransparentReceivedOutputSpends(BTreeMap<OutPoint, TxId>);

impl TransparentReceivedOutputSpends {
    pub fn new() -> Self {
        Self(BTreeMap::new())
    }
}

// transparent_received_outputs
pub struct ReceivedTransparentOutput {
    // Reference to the transaction in which this TXO was created
    pub(crate) transaction_id: TxId,
    // The account that controls spend authority for this TXO
    pub(crate) account_id: AccountId,
    // The address to which this TXO was sent
    pub(crate) address: TransparentAddress,
    // script, value_zat
    pub(crate) txout: TxOut,
}
