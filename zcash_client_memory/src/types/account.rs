use crate::{AccountId, ScanQueue};
use core::time;
use incrementalmerkletree::{Address, Marking, Retention};
use sapling::NullifierDerivingKey;
use secrecy::{ExposeSecret, SecretVec};
use shardtree::{error::ShardTreeError, store::memory::MemoryShardStore, ShardTree};
use std::{
    cmp::Ordering,
    collections::{hash_map::Entry, BTreeMap, HashMap, HashSet},
    convert::Infallible,
    hash::Hash,
    num::NonZeroU32,
    ops::Deref,
};
use zcash_keys::keys::{AddressGenerationError, DerivationError, UnifiedIncomingViewingKey};
use zip32::{fingerprint::SeedFingerprint, DiversifierIndex, Scope};

use zcash_primitives::{
    block::BlockHash,
    consensus::{BlockHeight, Network},
    transaction::{components::OutPoint, txid, Authorized, Transaction, TransactionData, TxId},
};
use zcash_protocol::{
    memo::{self, Memo, MemoBytes},
    value::{ZatBalance, Zatoshis},
    PoolType,
    ShieldedProtocol::{Orchard, Sapling},
};

use zcash_client_backend::{
    address::UnifiedAddress,
    data_api::{
        chain::ChainState, Account as _, AccountPurpose, AccountSource, SeedRelevance,
        TransactionDataRequest, TransactionStatus,
    },
    keys::{UnifiedAddressRequest, UnifiedFullViewingKey, UnifiedSpendingKey},
    proto::service::ShieldedProtocol,
    wallet::{Note, NoteId, WalletSaplingOutput, WalletSpend, WalletTransparentOutput, WalletTx},
};

use zcash_client_backend::data_api::{
    chain::CommitmentTreeRoot, scanning::ScanRange, AccountBirthday, BlockMetadata,
    DecryptedTransaction, NullifierQuery, ScannedBlock, SentTransaction, WalletCommitmentTrees,
    WalletRead, WalletSummary, WalletWrite, SAPLING_SHARD_HEIGHT,
};

#[cfg(feature = "transparent-inputs")]
use {
    zcash_client_backend::wallet::TransparentAddressMetadata,
    zcash_primitives::legacy::TransparentAddress,
};

#[cfg(feature = "orchard")]
use zcash_client_backend::{data_api::ORCHARD_SHARD_HEIGHT, wallet::WalletOrchardOutput};

use crate::error::Error;

/// An account stored in a `zcash_client_sqlite` database.
#[derive(Debug, Clone)]
pub struct Account {
    account_id: AccountId,
    kind: AccountSource,
    viewing_key: ViewingKey,
    birthday: AccountBirthday,
    purpose: AccountPurpose, // TODO: Remove this. AccountSource should be sufficient.
    addresses: BTreeMap<DiversifierIndex, UnifiedAddress>,
    notes: HashSet<NoteId>,
}

/// The viewing key that an [`Account`] has available to it.
#[derive(Debug, Clone)]
pub(crate) enum ViewingKey {
    /// A full viewing key.
    ///
    /// This is available to derived accounts, as well as accounts directly imported as
    /// full viewing keys.
    Full(Box<UnifiedFullViewingKey>),

    /// An incoming viewing key.
    ///
    /// Accounts that have this kind of viewing key cannot be used in wallet contexts,
    /// because they are unable to maintain an accurate balance.
    Incoming(Box<UnifiedIncomingViewingKey>),
}

impl Account {
    pub(crate) fn new(
        account_id: AccountId,
        kind: AccountSource,
        viewing_key: ViewingKey,
        birthday: AccountBirthday,
        purpose: AccountPurpose,
    ) -> Result<Self, Error> {
        let mut acc = Self {
            account_id,
            kind,
            viewing_key,
            birthday,
            purpose,
            addresses: BTreeMap::new(),
            notes: HashSet::new(),
        };
        let ua_request = acc
            .viewing_key
            .uivk()
            .to_address_request()
            .and_then(|ua_request| ua_request.intersect(&UnifiedAddressRequest::all().unwrap()))
            .ok_or_else(|| {
                Error::AddressGeneration(AddressGenerationError::ShieldedReceiverRequired)
            })?;

        let (addr, diversifier_index) = acc.default_address(ua_request)?;
        acc.addresses.insert(diversifier_index, addr);
        Ok(acc)
    }
    /// Returns the default Unified Address for the account,
    /// along with the diversifier index that generated it.
    ///
    /// The diversifier index may be non-zero if the Unified Address includes a Sapling
    /// receiver, and there was no valid Sapling receiver at diversifier index zero.
    pub(crate) fn default_address(
        &self,
        request: UnifiedAddressRequest,
    ) -> Result<(UnifiedAddress, DiversifierIndex), AddressGenerationError> {
        self.uivk().default_address(request)
    }

    pub(crate) fn birthday(&self) -> &AccountBirthday {
        &self.birthday
    }

    pub(crate) fn addresses(&self) -> &BTreeMap<DiversifierIndex, UnifiedAddress> {
        &self.addresses
    }

    pub(crate) fn current_address(&self) -> Option<(DiversifierIndex, UnifiedAddress)> {
        self.addresses
            .last_key_value()
            .map(|(diversifier_index, address)| (*diversifier_index, address.clone()))
    }
    pub(crate) fn kind(&self) -> &AccountSource {
        &self.kind
    }
    pub(crate) fn viewing_key(&self) -> &ViewingKey {
        &self.viewing_key
    }
    pub(crate) fn next_available_address(
        &mut self,
        request: UnifiedAddressRequest,
    ) -> Result<Option<UnifiedAddress>, Error> {
        match self.ufvk() {
            Some(ufvk) => {
                let search_from = match self.current_address() {
                    Some((mut last_diversifier_index, _)) => {
                        last_diversifier_index
                            .increment()
                            .map_err(|_| AddressGenerationError::DiversifierSpaceExhausted)?;
                        last_diversifier_index
                    }
                    None => DiversifierIndex::default(),
                };
                let (addr, diversifier_index) = ufvk.find_address(search_from, request)?;
                self.addresses.insert(diversifier_index, addr.clone());
                Ok(Some(addr))
            }
            None => Ok(None),
        }
    }
}

impl zcash_client_backend::data_api::Account<AccountId> for Account {
    fn id(&self) -> AccountId {
        self.account_id
    }

    fn source(&self) -> AccountSource {
        self.kind
    }

    fn ufvk(&self) -> Option<&UnifiedFullViewingKey> {
        self.viewing_key.ufvk()
    }

    fn uivk(&self) -> UnifiedIncomingViewingKey {
        self.viewing_key.uivk()
    }
}

impl ViewingKey {
    fn ufvk(&self) -> Option<&UnifiedFullViewingKey> {
        match self {
            ViewingKey::Full(ufvk) => Some(ufvk),
            ViewingKey::Incoming(_) => None,
        }
    }

    fn uivk(&self) -> UnifiedIncomingViewingKey {
        match self {
            ViewingKey::Full(ufvk) => ufvk.as_ref().to_unified_incoming_viewing_key(),
            ViewingKey::Incoming(uivk) => uivk.as_ref().clone(),
        }
    }
}