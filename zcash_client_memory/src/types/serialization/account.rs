use std::io;

use serde::{Deserialize, Serialize};
use serde_with::serde_as;
use serde_with::{DeserializeAs, TryFromInto};
use serde_with::{FromInto, SerializeAs};

use zcash_client_backend::data_api::{AccountPurpose, AccountSource};
use zcash_keys::keys::UnifiedFullViewingKey;

use zcash_primitives::block::BlockHash;
use zcash_protocol::consensus::{BlockHeight, MainNetwork};

use incrementalmerkletree::frontier::Frontier;
use serde::Serializer;

use zip32::fingerprint::SeedFingerprint;

use zcash_client_backend::data_api::{chain::ChainState, AccountBirthday};

use crate::{ByteArray, FrontierDef, ToFromBytes};

use super::{FromArray, ToArray};

/// Non trival conversion between Ufvk and Bytes.
/// There is no canonical way to convert a Ufvk to bytes, so we use the string encoding
/// over some fixed network and then convert that to bytes.
impl ToFromBytes for UnifiedFullViewingKey {
    fn to_bytes(&self) -> Vec<u8> {
        self.encode(&MainNetwork).as_bytes().to_vec()
    }

    fn from_bytes(bytes: &[u8]) -> io::Result<Self> {
        let b = std::str::from_utf8(bytes)
            .map_err(|_| io::Error::new(io::ErrorKind::InvalidData, "Invalid utf8"))?;
        UnifiedFullViewingKey::decode(&MainNetwork, b).map_err(|_| {
            io::Error::new(
                io::ErrorKind::InvalidData,
                "Invalid unified full viewing key",
            )
        })
    }
}

#[serde_as]
#[derive(Serialize, Deserialize)]
#[serde(remote = "AccountPurpose")]
pub enum AccountPurposeDef {
    /// For spending accounts, the wallet will track information needed to spend
    /// received notes.
    Spending,
    /// For view-only accounts, the wallet will not track spend information.
    ViewOnly,
}

#[serde_as]
#[derive(Serialize, Deserialize)]
#[serde(remote = "zcash_client_backend::data_api::chain::ChainState")]
struct ChainStateDef {
    #[serde_as(as = "FromInto<u32>")]
    #[serde(getter = "zcash_client_backend::data_api::chain::ChainState::block_height")]
    pub block_height: BlockHeight,
    #[serde_as(as = "ByteArray<32>")]
    #[serde(getter = "zcash_client_backend::data_api::chain::ChainState::block_hash")]
    pub block_hash: BlockHash,
    #[serde_as(as = "FrontierDef")]
    #[serde(getter = "zcash_client_backend::data_api::chain::ChainState::final_sapling_tree")]
    pub final_sapling_tree: Frontier<sapling::Node, { sapling::NOTE_COMMITMENT_TREE_DEPTH }>,
    #[cfg(feature = "orchard")]
    #[serde_as(as = "FrontierDef")]
    #[serde(getter = "zcash_client_backend::data_api::chain::ChainState::final_orchard_tree")]
    pub final_orchard_tree:
        Frontier<orchard::tree::MerkleHashOrchard, { orchard::NOTE_COMMITMENT_TREE_DEPTH as u8 }>,
}

#[serde_as]
#[derive(Serialize, Deserialize)]
#[serde(remote = "zcash_client_backend::data_api::AccountBirthday")]
pub struct AccountBirthdayDef {
    #[serde_as(as = "ChainStateDef")]
    #[serde(getter = "zcash_client_backend::data_api::AccountBirthday::prior_chain_state")]
    pub prior_chain_state: ChainState,
    #[serde_as(as = "Option<FromInto<u32>>")]
    #[serde(getter = "zcash_client_backend::data_api::AccountBirthday::recover_until")]
    pub recover_until: Option<BlockHeight>,
}

#[serde_as]
#[derive(Serialize, Deserialize)]
#[serde(remote = "AccountSource")]
pub enum AccountSourceDef {
    /// An account derived from a known seed.
    Derived {
        #[serde_as(as = "ByteArray<32>")]
        seed_fingerprint: SeedFingerprint,
        #[serde_as(as = "TryFromInto<u32>")]
        account_index: zip32::AccountId,
    },

    /// An account imported from a viewing key.
    Imported {
        #[serde_as(as = "AccountPurposeDef")]
        purpose: AccountPurpose,
    },
}

// BOILERPLATE: Trivial conversions between types and the trivial implementations of SerializeAs and DeserializeAs

impl SerializeAs<ChainState> for ChainStateDef {
    fn serialize_as<S>(source: &ChainState, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: Serializer,
    {
        ChainStateDef::serialize(source, serializer)
    }
}

impl<'de> DeserializeAs<'de, ChainState> for ChainStateDef {
    fn deserialize_as<D>(deserializer: D) -> Result<ChainState, D::Error>
    where
        D: serde::Deserializer<'de>,
    {
        ChainStateDef::deserialize(deserializer)
    }
}

impl From<ChainStateDef> for zcash_client_backend::data_api::chain::ChainState {
    fn from(wrapper: ChainStateDef) -> Self {
        Self::new(
            wrapper.block_height,
            wrapper.block_hash,
            wrapper.final_sapling_tree,
            #[cfg(feature = "orchard")]
            wrapper.final_orchard_tree,
        )
    }
}

impl From<AccountBirthdayDef> for zcash_client_backend::data_api::AccountBirthday {
    fn from(wrapper: AccountBirthdayDef) -> Self {
        Self::from_parts(
            wrapper.prior_chain_state,
            wrapper.recover_until.map(Into::into),
        )
    }
}

// Provide a conversion to construct the remote type.
impl From<AccountSourceDef> for AccountSource {
    fn from(def: AccountSourceDef) -> AccountSource {
        match def {
            AccountSourceDef::Derived {
                seed_fingerprint,
                account_index,
            } => AccountSource::Derived {
                seed_fingerprint,
                account_index,
            },
            AccountSourceDef::Imported { purpose } => AccountSource::Imported { purpose },
        }
    }
}

impl SerializeAs<AccountBirthday> for AccountBirthdayDef {
    fn serialize_as<S>(source: &AccountBirthday, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: Serializer,
    {
        AccountBirthdayDef::serialize(source, serializer)
    }
}

impl<'de> DeserializeAs<'de, AccountBirthday> for AccountBirthdayDef {
    fn deserialize_as<D>(deserializer: D) -> Result<AccountBirthday, D::Error>
    where
        D: serde::Deserializer<'de>,
    {
        AccountBirthdayDef::deserialize(deserializer)
    }
}

impl serde_with::SerializeAs<AccountSource> for AccountSourceDef {
    fn serialize_as<S>(value: &AccountSource, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: serde::Serializer,
    {
        AccountSourceDef::serialize(value, serializer)
    }
}

impl<'de> serde_with::DeserializeAs<'de, AccountSource> for AccountSourceDef {
    fn deserialize_as<D>(deserializer: D) -> Result<AccountSource, D::Error>
    where
        D: serde::Deserializer<'de>,
    {
        AccountSourceDef::deserialize(deserializer).map(Into::into)
    }
}

impl serde_with::SerializeAs<AccountPurpose> for AccountPurposeDef {
    fn serialize_as<S>(value: &AccountPurpose, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: serde::Serializer,
    {
        AccountPurposeDef::serialize(value, serializer)
    }
}
impl<'de> serde_with::DeserializeAs<'de, AccountPurpose> for AccountPurposeDef {
    fn deserialize_as<D>(deserializer: D) -> Result<AccountPurpose, D::Error>
    where
        D: serde::Deserializer<'de>,
    {
        AccountPurposeDef::deserialize(deserializer)
    }
}
impl ToArray<u8, 32> for SeedFingerprint {
    fn to_array(&self) -> [u8; 32] {
        self.to_bytes()
    }
}
impl FromArray<u8, 32> for SeedFingerprint {
    fn from_array(arr: [u8; 32]) -> Self {
        Self::from_bytes(arr)
    }
}
