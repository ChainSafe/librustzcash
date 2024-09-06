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

use super::ToFromBytes;
use crate::BlockHashWrapper;
use crate::FrontierWrapper;

pub(crate) struct AccountPurposeWrapper;
impl serde_with::SerializeAs<AccountPurpose> for AccountPurposeWrapper {
    fn serialize_as<S>(value: &AccountPurpose, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: serde::Serializer,
    {
        match value {
            AccountPurpose::Spending => serializer.serialize_str("Spending"),
            AccountPurpose::ViewOnly => serializer.serialize_str("ViewOnly"),
        }
    }
}
impl<'de> serde_with::DeserializeAs<'de, AccountPurpose> for AccountPurposeWrapper {
    fn deserialize_as<D>(deserializer: D) -> Result<AccountPurpose, D::Error>
    where
        D: serde::Deserializer<'de>,
    {
        let s = String::deserialize(deserializer)?;
        match s.as_str() {
            "Spending" => Ok(AccountPurpose::Spending),
            "ViewOnly" => Ok(AccountPurpose::ViewOnly),
            _ => Err(serde::de::Error::custom("Invalid account purpose")),
        }
    }
}
#[serde_as]
#[derive(Serialize, Deserialize)]
#[serde(remote = "zcash_client_backend::data_api::chain::ChainState")]
pub struct ChainStateWrapper {
    #[serde_as(as = "FromInto<u32>")]
    #[serde(getter = "zcash_client_backend::data_api::chain::ChainState::block_height")]
    pub block_height: BlockHeight,
    #[serde_as(as = "BlockHashWrapper")]
    #[serde(getter = "zcash_client_backend::data_api::chain::ChainState::block_hash")]
    pub block_hash: BlockHash,
    #[serde_as(as = "FrontierWrapper<sapling::Node>")]
    #[serde(getter = "zcash_client_backend::data_api::chain::ChainState::final_sapling_tree")]
    pub final_sapling_tree: Frontier<sapling::Node, { sapling::NOTE_COMMITMENT_TREE_DEPTH }>,
    #[cfg(feature = "orchard")]
    #[serde_as(as = "FrontierWrapper<orchard::tree::MerkleHashOrchard>")]
    #[serde(getter = "zcash_client_backend::data_api::chain::ChainState::final_orchard_tree")]
    pub final_orchard_tree:
        Frontier<orchard::tree::MerkleHashOrchard, { orchard::NOTE_COMMITMENT_TREE_DEPTH as u8 }>,
}
impl SerializeAs<ChainState> for ChainStateWrapper {
    fn serialize_as<S>(source: &ChainState, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: Serializer,
    {
        ChainStateWrapper::serialize(source, serializer)
    }
}

impl<'de> DeserializeAs<'de, ChainState> for ChainStateWrapper {
    fn deserialize_as<D>(deserializer: D) -> Result<ChainState, D::Error>
    where
        D: serde::Deserializer<'de>,
    {
        ChainStateWrapper::deserialize(deserializer)
    }
}

impl From<ChainStateWrapper> for zcash_client_backend::data_api::chain::ChainState {
    fn from(wrapper: ChainStateWrapper) -> Self {
        Self::new(
            wrapper.block_height,
            wrapper.block_hash,
            wrapper.final_sapling_tree,
            #[cfg(feature = "orchard")]
            wrapper.final_orchard_tree,
        )
    }
}

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
#[serde(remote = "zcash_client_backend::data_api::AccountBirthday")]
pub struct AccountBirthdayWrapper {
    #[serde_as(as = "ChainStateWrapper")]
    #[serde(getter = "zcash_client_backend::data_api::AccountBirthday::prior_chain_state")]
    pub prior_chain_state: ChainState,
    #[serde_as(as = "Option<FromInto<u32>>")]
    #[serde(getter = "zcash_client_backend::data_api::AccountBirthday::recover_until")]
    pub recover_until: Option<BlockHeight>,
}
impl SerializeAs<AccountBirthday> for AccountBirthdayWrapper {
    fn serialize_as<S>(source: &AccountBirthday, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: Serializer,
    {
        AccountBirthdayWrapper::serialize(source, serializer)
    }
}

impl<'de> DeserializeAs<'de, AccountBirthday> for AccountBirthdayWrapper {
    fn deserialize_as<D>(deserializer: D) -> Result<AccountBirthday, D::Error>
    where
        D: serde::Deserializer<'de>,
    {
        AccountBirthdayWrapper::deserialize(deserializer)
    }
}

impl From<AccountBirthdayWrapper> for zcash_client_backend::data_api::AccountBirthday {
    fn from(wrapper: AccountBirthdayWrapper) -> Self {
        Self::from_parts(
            wrapper.prior_chain_state,
            wrapper.recover_until.map(Into::into),
        )
    }
}

#[serde_as]
#[derive(Serialize, Deserialize)]
#[serde(remote = "AccountSource")]
pub enum AccountSourceWrapper {
    /// An account derived from a known seed.
    Derived {
        #[serde_as(as = "SeedFingerprintWrapper")]
        seed_fingerprint: SeedFingerprint,
        #[serde_as(as = "TryFromInto<u32>")]
        account_index: zip32::AccountId,
    },

    /// An account imported from a viewing key.
    Imported {
        #[serde_as(as = "AccountPurposeWrapper")]
        purpose: AccountPurpose,
    },
}
// Provide a conversion to construct the remote type.
impl From<AccountSourceWrapper> for AccountSource {
    fn from(def: AccountSourceWrapper) -> AccountSource {
        match def {
            AccountSourceWrapper::Derived {
                seed_fingerprint,
                account_index,
            } => AccountSource::Derived {
                seed_fingerprint,
                account_index,
            },
            AccountSourceWrapper::Imported { purpose } => AccountSource::Imported { purpose },
        }
    }
}

impl serde_with::SerializeAs<AccountSource> for AccountSourceWrapper {
    fn serialize_as<S>(value: &AccountSource, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: serde::Serializer,
    {
        AccountSourceWrapper::serialize(value, serializer)
    }
}

impl<'de> serde_with::DeserializeAs<'de, AccountSource> for AccountSourceWrapper {
    fn deserialize_as<D>(deserializer: D) -> Result<AccountSource, D::Error>
    where
        D: serde::Deserializer<'de>,
    {
        AccountSourceWrapper::deserialize(deserializer).map(Into::into)
    }
}
pub(crate) struct SeedFingerprintWrapper;
impl serde_with::SerializeAs<SeedFingerprint> for SeedFingerprintWrapper {
    fn serialize_as<S>(value: &SeedFingerprint, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: serde::Serializer,
    {
        value.to_bytes().serialize(serializer)
    }
}
impl<'de> serde_with::DeserializeAs<'de, SeedFingerprint> for SeedFingerprintWrapper {
    fn deserialize_as<D>(deserializer: D) -> Result<SeedFingerprint, D::Error>
    where
        D: serde::Deserializer<'de>,
    {
        Ok(SeedFingerprint::from_bytes(<[u8; 32]>::deserialize(
            deserializer,
        )?))
    }
}
