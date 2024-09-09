use serde::{Deserialize, Serialize};

use serde_with::serde_as;

use serde_with::FromInto;

use zcash_primitives::transaction::TxId;

use super::FromArray;
use super::ToArray;

impl ToArray<u8, 32> for TxId {
    fn to_array(&self) -> [u8; 32] {
        *self.as_ref()
    }
}

impl FromArray<u8, 32> for TxId {
    fn from_array(bytes: [u8; 32]) -> Self {
        TxId::from_bytes(bytes)
    }
}

#[serde_as]
#[derive(Serialize, Deserialize)]
#[serde(remote = "zcash_client_backend::data_api::TransactionStatus")]
pub enum TransactionStatusWrapper {
    /// The requested transaction ID was not recognized by the node.
    TxidNotRecognized,
    /// The requested transaction ID corresponds to a transaction that is recognized by the node,
    /// but is in the mempool or is otherwise not mined in the main chain (but may have been mined
    /// on a fork that was reorged away).
    NotInMainChain,
    /// The requested transaction ID corresponds to a transaction that has been included in the
    /// block at the provided height.
    Mined(#[serde_as(as = "FromInto<u32>")] zcash_primitives::consensus::BlockHeight),
}
