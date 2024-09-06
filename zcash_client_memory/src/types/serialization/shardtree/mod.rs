mod frontier;
mod tree;
pub use frontier::*;
use sapling::{value::NoteValue, PaymentAddress, Rseed};
use serde::de::VariantAccess;
use serde_with::{de::DeserializeAsWrap, DeserializeAs, SerializeAs};
use std::collections::BTreeSet;
pub use tree::*;
use zcash_client_backend::wallet::Note;
use zip32::Scope;

use std::io;
use std::ops::Deref;
use std::sync::Arc;

use incrementalmerkletree::Hashable;
use serde::ser::{SerializeSeq, SerializeTuple};
use serde::Deserializer;
use serde::{ser::SerializeStruct, Deserialize, Serialize};
use serde_with::FromInto;
use serde_with::TryFromInto;
use serde_with::{ser::SerializeAsWrap, serde_as};
use shardtree::store::memory::MemoryShardStore;
use shardtree::store::{Checkpoint, TreeState};
use shardtree::RetentionFlags;
use shardtree::{store::ShardStore, LocatedPrunableTree, Node as TreeNode, PrunableTree};
use std::fmt::Debug;
use zcash_client_backend::data_api::scanning::ScanPriority;
use zcash_client_backend::{
    data_api::{AccountPurpose, AccountSource},
    wallet::NoteId,
};
use zcash_keys::keys::UnifiedFullViewingKey;

use zcash_primitives::{block::BlockHash, transaction::TxId};
use zcash_protocol::consensus::{BlockHeight, MainNetwork};

use crate::ToFromBytes;
use crate::ToFromBytesWrapper;
use crate::TxIdWrapper;
use zcash_protocol::memo::Memo;
use zcash_protocol::{memo::MemoBytes, ShieldedProtocol};
use zip32::fingerprint::SeedFingerprint;
impl ToFromBytes for sapling::Node {
    fn to_bytes(&self) -> Vec<u8> {
        self.to_bytes().to_vec()
    }

    fn from_bytes(bytes: &[u8]) -> io::Result<Self> {
        let repr: [u8; 32] = bytes.try_into().map_err(|_| {
            io::Error::new(
                io::ErrorKind::InvalidData,
                "Invalid length for Jubjub base field value.",
            )
        })?;
        Option::from(Self::from_bytes(repr)).ok_or_else(|| {
            io::Error::new(
                io::ErrorKind::InvalidData,
                "Non-canonical encoding of Jubjub base field value.",
            )
        })
    }
}

#[cfg(feature = "orchard")]
impl ToFromBytes for orchard::tree::MerkleHashOrchard {
    fn to_bytes(&self) -> Vec<u8> {
        self.to_bytes().to_vec()
    }

    fn from_bytes(bytes: &[u8]) -> io::Result<Self> {
        let repr: [u8; 32] = bytes.try_into().map_err(|_| {
            io::Error::new(
                io::ErrorKind::InvalidData,
                "Invalid length for Pallas base field value.",
            )
        })?;
        <Option<_>>::from(Self::from_bytes(&repr)).ok_or_else(|| {
            io::Error::new(
                io::ErrorKind::InvalidData,
                "Non-canonical encoding of Pallas base field value.",
            )
        })
    }
}
