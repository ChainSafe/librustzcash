use serde::{Deserialize, Serialize};
use serde_with::serde_as;
use std::{
    collections::{BTreeMap, BTreeSet},
    ops::{Deref, DerefMut},
};
use subtle::ConditionallySelectable;
use zcash_keys::keys::{AddressGenerationError, UnifiedIncomingViewingKey};
use zip32::DiversifierIndex;

use crate::error::Error;
use crate::serialization::*;

use zcash_client_backend::data_api::AccountBirthday;
use zcash_client_backend::{
    address::UnifiedAddress,
    data_api::{Account as _, AccountPurpose, AccountSource},
    keys::{UnifiedAddressRequest, UnifiedFullViewingKey},
    wallet::NoteId,
};
/// Internal representation of ID type for accounts. Will be unique for each account.
#[derive(
    Debug, Copy, Clone, PartialEq, Eq, Hash, Default, PartialOrd, Ord, Serialize, Deserialize,
)]
pub struct AccountId(u32);

impl From<u32> for AccountId {
    fn from(id: u32) -> Self {
        AccountId(id)
    }
}

impl Deref for AccountId {
    type Target = u32;

    fn deref(&self) -> &Self::Target {
        &self.0
    }
}

impl ConditionallySelectable for AccountId {
    fn conditional_select(a: &Self, b: &Self, choice: subtle::Choice) -> Self {
        AccountId(ConditionallySelectable::conditional_select(
            &a.0, &b.0, choice,
        ))
    }
}

/// This is the top-level struct that handles accounts. We could theoretically have this just be a Vec
/// but we want to have control over the internal AccountId values. The account ids are unique.
#[derive(Serialize, Deserialize)]
pub(crate) struct Accounts {
    nonce: u32,
    accounts: BTreeMap<AccountId, Account>,
}

impl Accounts {
    pub(crate) fn new() -> Self {
        Self {
            nonce: 0,
            accounts: BTreeMap::new(),
        }
    }

    /// Creates a new account. The account id will be determined by the internal nonce.
    /// Do not call this directly, use the `Wallet` methods instead.
    /// Otherwise the scan queue will not be correctly updated
    pub(crate) fn new_account(
        &mut self,
        kind: AccountSource,
        viewing_key: UnifiedFullViewingKey,
        birthday: AccountBirthday,
        purpose: AccountPurpose,
    ) -> Result<(AccountId, Account), Error> {
        let account_id = AccountId(self.nonce);

        let acc = Account::new(account_id, kind, viewing_key, birthday, purpose)?;

        self.nonce += 1;
        self.accounts.insert(account_id, acc.clone());

        Ok((account_id, acc))
    }

    pub(crate) fn get(&self, account_id: AccountId) -> Option<&Account> {
        self.accounts.get(&account_id)
    }

    pub(crate) fn get_mut(&mut self, account_id: AccountId) -> Option<&mut Account> {
        self.accounts.get_mut(&account_id)
    }
    /// Gets the account ids of all accounts
    pub(crate) fn account_ids(&self) -> impl Iterator<Item = &AccountId> {
        self.accounts.keys()
    }
}

impl Deref for Accounts {
    type Target = BTreeMap<AccountId, Account>;

    fn deref(&self) -> &Self::Target {
        &self.accounts
    }
}

impl DerefMut for Accounts {
    fn deref_mut(&mut self) -> &mut Self::Target {
        &mut self.accounts
    }
}

/// An internal representation account stored in the database.
#[serde_as]
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Account {
    account_id: AccountId,

    #[serde_as(as = "AccountSourceDef")]
    kind: AccountSource,

    #[serde_as(as = "BytesVec<UnifiedFullViewingKey>")]
    viewing_key: UnifiedFullViewingKey,

    #[serde_as(as = "AccountBirthdayDef")]
    birthday: AccountBirthday,

    #[serde_as(as = "AccountPurposeDef")]
    _purpose: AccountPurpose, // TODO: Remove this. AccountSource should be sufficient.

    /// Stores diversified Unified Addresses that have been generated from accounts in the wallet.
    #[serde(skip)]
    addresses: BTreeMap<DiversifierIndex, UnifiedAddress>,

    #[serde_as(as = "BTreeSet<NoteIdDef>")]
    _notes: BTreeSet<NoteId>,
}

impl Account {
    pub(crate) fn new(
        account_id: AccountId,
        kind: AccountSource,
        viewing_key: UnifiedFullViewingKey,
        birthday: AccountBirthday,
        purpose: AccountPurpose,
    ) -> Result<Self, Error> {
        let mut acc = Self {
            account_id,
            kind,
            viewing_key,
            birthday,
            _purpose: purpose,
            addresses: BTreeMap::new(),
            _notes: BTreeSet::new(),
        };

        // populate the addresses map with the default address
        let ua_request = acc
            .viewing_key
            .to_unified_incoming_viewing_key()
            .to_address_request()
            .and_then(|ua_request| ua_request.intersect(&UnifiedAddressRequest::all().unwrap()))
            .ok_or_else(|| {
                Error::AddressGeneration(AddressGenerationError::ShieldedReceiverRequired)
            })?;
        let (ua, diversifier_index) = acc.default_address(ua_request)?;
        acc.addresses.insert(diversifier_index, ua);

        Ok(acc)
    }

    pub fn addresses(&self) -> &BTreeMap<DiversifierIndex, UnifiedAddress> {
        &self.addresses
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

    pub(crate) fn current_address(&self) -> Result<(UnifiedAddress, DiversifierIndex), Error> {
        Ok(self
            .addresses
            .iter()
            .last()
            .map(|(diversifier_index, ua)| (ua.clone(), *diversifier_index))
            .unwrap()) // can unwrap as the map is never empty
    }

    pub(crate) fn kind(&self) -> &AccountSource {
        &self.kind
    }

    pub(crate) fn next_available_address(
        &mut self,
        request: UnifiedAddressRequest,
    ) -> Result<Option<UnifiedAddress>, Error> {
        match self.ufvk() {
            Some(ufvk) => {
                let search_from = self
                    .current_address()
                    .map(|(_, mut diversifier_index)| {
                        diversifier_index.increment().map_err(|_| {
                            Error::AddressGeneration(
                                AddressGenerationError::DiversifierSpaceExhausted,
                            )
                        })?;
                        Ok::<_, Error>(diversifier_index)
                    })
                    .unwrap_or(Ok(DiversifierIndex::default()))?;
                let (ua, diversifier_index) = ufvk.find_address(search_from, request)?;
                self.addresses.insert(diversifier_index, ua.clone());
                Ok(Some(ua))
            }
            None => Ok(None),
        }
    }

    pub(crate) fn account_id(&self) -> AccountId {
        self.account_id
    }
}

impl zcash_client_backend::data_api::Account for Account {
    type AccountId = AccountId;

    fn id(&self) -> AccountId {
        self.account_id
    }

    fn source(&self) -> AccountSource {
        self.kind
    }

    fn ufvk(&self) -> Option<&UnifiedFullViewingKey> {
        Some(&self.viewing_key)
    }

    fn uivk(&self) -> UnifiedIncomingViewingKey {
        self.viewing_key.to_unified_incoming_viewing_key()
    }
}
