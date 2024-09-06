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

pub struct MemoBytesWrapper;
impl serde_with::SerializeAs<MemoBytes> for MemoBytesWrapper {
    fn serialize_as<S>(value: &MemoBytes, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: serde::Serializer,
    {
        value.as_slice().serialize(serializer)
    }
}

impl<'de> serde_with::DeserializeAs<'de, MemoBytes> for MemoBytesWrapper {
    fn deserialize_as<D>(deserializer: D) -> Result<MemoBytes, D::Error>
    where
        D: serde::Deserializer<'de>,
    {
        let b = <Vec<u8>>::deserialize(deserializer)?;
        MemoBytes::from_bytes(&b).map_err(|_| serde::de::Error::custom("Invalid memo bytes"))
    }
}

impl serde_with::SerializeAs<Memo> for MemoBytesWrapper {
    fn serialize_as<S>(value: &Memo, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: serde::Serializer,
    {
        value.encode().as_slice().serialize(serializer)
    }
}

impl<'de> serde_with::DeserializeAs<'de, Memo> for MemoBytesWrapper {
    fn deserialize_as<D>(deserializer: D) -> Result<Memo, D::Error>
    where
        D: serde::Deserializer<'de>,
    {
        let b = <Vec<u8>>::deserialize(deserializer)?;
        Memo::from_bytes(&b).map_err(|_| serde::de::Error::custom("Invalid memo"))
    }
}
