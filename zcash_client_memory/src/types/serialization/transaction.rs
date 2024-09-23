use serde::Deserializer;
use serde::{Deserialize, Serialize};
use zcash_protocol::consensus::BlockHeight;

use super::{ByteArray, FromArray, ToArray, TransparentAddressDef};
use serde_with::{serde_as, DeserializeAs, FromInto, SerializeAs, TryFromInto};
use zcash_client_backend::data_api::TransactionDataRequest;
use zcash_primitives::legacy::{Script, TransparentAddress};
use zcash_primitives::transaction::components::amount::NonNegativeAmount;
use zcash_primitives::transaction::components::TxOut;
use zcash_primitives::transaction::TxId;
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
pub enum TransactionStatusDef {
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

#[serde_as]
#[derive(Serialize, Deserialize)]
#[serde(remote = "TxOut")]
pub struct TxOutDef {
    #[serde_as(as = "TryFromInto<u64>")]
    pub value: NonNegativeAmount,
    #[serde_as(as = "ScriptDef")]
    pub script_pubkey: Script,
}

impl SerializeAs<TxOut> for TxOutDef {
    fn serialize_as<S>(value: &TxOut, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: serde::Serializer,
    {
        TxOutDef::serialize(value, serializer)
    }
}

impl<'de> DeserializeAs<'de, TxOut> for TxOutDef {
    fn deserialize_as<D>(deserializer: D) -> Result<TxOut, D::Error>
    where
        D: Deserializer<'de>,
    {
        TxOutDef::deserialize(deserializer)
    }
}

#[derive(Serialize, Deserialize)]
#[serde(remote = "Script")]
pub struct ScriptDef(pub Vec<u8>);
impl SerializeAs<Script> for ScriptDef {
    fn serialize_as<S>(value: &Script, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: serde::Serializer,
    {
        ScriptDef::serialize(value, serializer)
    }
}

impl<'de> DeserializeAs<'de, Script> for ScriptDef {
    fn deserialize_as<D>(deserializer: D) -> Result<Script, D::Error>
    where
        D: Deserializer<'de>,
    {
        ScriptDef::deserialize(deserializer)
    }
}

#[serde_as]
#[derive(Serialize, Deserialize)]
#[serde(remote = "TransactionDataRequest")]
pub enum TransactionDataRequestDef {
    /// Information about the chain's view of a transaction is requested.
    ///
    /// The caller evaluating this request on behalf of the wallet backend should respond to this
    /// request by determining the status of the specified transaction with respect to the main
    /// chain; if using `lightwalletd` for access to chain data, this may be obtained by
    /// interpreting the results of the [`GetTransaction`] RPC method. It should then call
    /// [`WalletWrite::set_transaction_status`] to provide the resulting transaction status
    /// information to the wallet backend.
    ///
    /// [`GetTransaction`]: crate::proto::service::compact_tx_streamer_client::CompactTxStreamerClient::get_transaction
    GetStatus(#[serde_as(as = "ByteArray<32>")] TxId),
    /// Transaction enhancement (download of complete raw transaction data) is requested.
    ///
    /// The caller evaluating this request on behalf of the wallet backend should respond to this
    /// request by providing complete data for the specified transaction to
    /// [`wallet::decrypt_and_store_transaction`]; if using `lightwalletd` for access to chain
    /// state, this may be obtained via the [`GetTransaction`] RPC method. If no data is available
    /// for the specified transaction, this should be reported to the backend using
    /// [`WalletWrite::set_transaction_status`]. A [`TransactionDataRequest::Enhancement`] request
    /// subsumes any previously existing [`TransactionDataRequest::GetStatus`] request.
    ///
    /// [`GetTransaction`]: crate::proto::service::compact_tx_streamer_client::CompactTxStreamerClient::get_transaction
    Enhancement(#[serde_as(as = "ByteArray<32>")] TxId),
    /// Information about transactions that receive or spend funds belonging to the specified
    /// transparent address is requested.
    ///
    /// Fully transparent transactions, and transactions that do not contain either shielded inputs
    /// or shielded outputs belonging to the wallet, may not be discovered by the process of chain
    /// scanning; as a consequence, the wallet must actively query to find transactions that spend
    /// such funds. Ideally we'd be able to query by [`OutPoint`] but this is not currently
    /// functionality that is supported by the light wallet server.
    ///
    /// The caller evaluating this request on behalf of the wallet backend should respond to this
    /// request by detecting transactions involving the specified address within the provided block
    /// range; if using `lightwalletd` for access to chain data, this may be performed using the
    /// [`GetTaddressTxids`] RPC method. It should then call [`wallet::decrypt_and_store_transaction`]
    /// for each transaction so detected.
    ///
    /// [`GetTaddressTxids`]: crate::proto::service::compact_tx_streamer_client::CompactTxStreamerClient::get_taddress_txids
    #[cfg(feature = "transparent-inputs")]
    SpendsFromAddress {
        #[serde_as(as = "TransparentAddressDef")]
        address: TransparentAddress,
        #[serde_as(as = "FromInto<u32>")]
        block_range_start: BlockHeight,
        #[serde_as(as = "Option<FromInto<u32>>")]
        block_range_end: Option<BlockHeight>,
    },
}

impl SerializeAs<TransactionDataRequest> for TransactionDataRequestDef {
    fn serialize_as<S>(value: &TransactionDataRequest, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: serde::Serializer,
    {
        TransactionDataRequestDef::serialize(value, serializer)
    }
}

impl<'de> DeserializeAs<'de, TransactionDataRequest> for TransactionDataRequestDef {
    fn deserialize_as<D>(deserializer: D) -> Result<TransactionDataRequest, D::Error>
    where
        D: Deserializer<'de>,
    {
        TransactionDataRequestDef::deserialize(deserializer)
    }
}
