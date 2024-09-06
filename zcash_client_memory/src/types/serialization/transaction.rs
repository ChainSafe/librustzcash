use std::collections::BTreeSet;

use std::io;
use std::ops::Deref;
use std::sync::Arc;

use incrementalmerkletree::frontier::Frontier;
use incrementalmerkletree::Hashable;
use serde::ser::{SerializeSeq, SerializeTuple};
use serde::{ser::SerializeStruct, Deserialize, Serialize};
use serde::{Deserializer, Serializer};
use serde_with::{ser::SerializeAsWrap, serde_as};
use serde_with::{DeserializeAs, TryFromInto};
use serde_with::{FromInto, SerializeAs};

use std::fmt::Debug;
use zcash_client_backend::data_api::chain::ChainState;
use zcash_client_backend::data_api::scanning::ScanPriority;
use zcash_client_backend::{
    data_api::{AccountPurpose, AccountSource},
    wallet::NoteId,
};
use zcash_keys::keys::UnifiedFullViewingKey;

use zcash_primitives::{block::BlockHash, transaction::TxId};
use zcash_protocol::consensus::{BlockHeight, MainNetwork};

use zcash_protocol::memo::Memo;
use zcash_protocol::{memo::MemoBytes, ShieldedProtocol};
use zip32::fingerprint::SeedFingerprint;

pub(crate) struct TxIdWrapper;

impl serde_with::SerializeAs<TxId> for TxIdWrapper {
    fn serialize_as<S>(value: &TxId, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: serde::Serializer,
    {
        value.as_ref().serialize(serializer)
    }
}

impl<'de> serde_with::DeserializeAs<'de, TxId> for TxIdWrapper {
    fn deserialize_as<D>(deserializer: D) -> Result<TxId, D::Error>
    where
        D: serde::Deserializer<'de>,
    {
        Ok(TxId::from_bytes(<[u8; 32]>::deserialize(deserializer)?))
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
