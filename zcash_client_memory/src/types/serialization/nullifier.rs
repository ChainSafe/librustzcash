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

use crate::ToFromBytes;
use zcash_protocol::memo::Memo;
use zcash_protocol::{memo::MemoBytes, ShieldedProtocol};
use zip32::fingerprint::SeedFingerprint;
impl ToFromBytes for sapling::Nullifier {
    fn to_bytes(&self) -> Vec<u8> {
        self.0.to_vec()
    }

    fn from_bytes(bytes: &[u8]) -> io::Result<Self> {
        Ok(sapling::Nullifier(bytes.try_into().map_err(|e| {
            io::Error::new(io::ErrorKind::InvalidData, format!("{}", e))
        })?))
    }
}

#[cfg(feature = "orchard")]
impl ToFromBytes for orchard::note::Nullifier {
    fn to_bytes(&self) -> Vec<u8> {
        (*self).to_bytes().to_vec()
    }

    fn from_bytes(bytes: &[u8]) -> io::Result<Self> {
        orchard::note::Nullifier::from_bytes(
            bytes
                .try_into()
                .map_err(|e| io::Error::new(io::ErrorKind::InvalidData, format!("{}", e)))?,
        )
        .into_option()
        .ok_or_else(|| io::Error::new(io::ErrorKind::InvalidData, "Invalid sapling nullifier"))
    }
}
