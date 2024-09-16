use serde::{Deserialize, Deserializer, Serialize};

use serde_with::{serde_as, TryFromInto};
use serde_with::{DeserializeAs, SerializeAs};

use super::FromArray;
use super::ToArray;
use serde_with::FromInto;
use zcash_primitives::legacy::Script;
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
