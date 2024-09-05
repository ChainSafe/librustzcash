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
// #[derive(Serialize, Deserialize)]
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
    pub(crate) fn new_account(
        &mut self,
        kind: AccountSource,
        viewing_key: UnifiedFullViewingKey,
        birthday: AccountBirthday,
        purpose: AccountPurpose,
    ) -> Result<(AccountId, Account), Error> {
        let account_id = AccountId(self.nonce);
        let mut acc = Account {
            account_id,
            kind,
            viewing_key,
            birthday,
            _purpose: purpose,
            addresses: BTreeMap::new(),
            _notes: BTreeSet::new(),
        };
        let ua_request = acc
            .viewing_key
            .to_unified_incoming_viewing_key()
            .to_address_request()
            .and_then(|ua_request| ua_request.intersect(&UnifiedAddressRequest::all().unwrap()))
            .ok_or_else(|| {
                Error::AddressGeneration(AddressGenerationError::ShieldedReceiverRequired)
            })?;

        let (addr, diversifier_index) = acc.default_address(ua_request)?;
        acc.addresses.insert(diversifier_index, addr);

        self.accounts.insert(account_id, acc.clone());
        self.nonce += 1;
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

impl IntoIterator for Accounts {
    type Item = (AccountId, Account);
    type IntoIter = std::collections::btree_map::IntoIter<AccountId, Account>;

    fn into_iter(self) -> Self::IntoIter {
        self.accounts.into_iter()
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
#[derive(Debug, Clone, Serialize)]
pub struct Account {
    account_id: AccountId,
    #[serde_as(as = "AccountSourceWrapper")]
    kind: AccountSource,
    #[serde(skip)]
    viewing_key: UnifiedFullViewingKey,
    #[serde_as(as = "AccountBirthdayWrapper")]
    birthday: AccountBirthday,
    #[serde_as(as = "AccountPurposeWrapper")]
    _purpose: AccountPurpose, // TODO: Remove this. AccountSource should be sufficient.
    #[serde(skip)]
    addresses: BTreeMap<DiversifierIndex, UnifiedAddress>,
    #[serde_as(as = "BTreeSet<NoteIdWrapper>")]
    _notes: BTreeSet<NoteId>,
}

impl Account {
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

    pub(crate) fn _addresses(&self) -> &BTreeMap<DiversifierIndex, UnifiedAddress> {
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

    pub(crate) fn account_id(&self) -> AccountId {
        self.account_id
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
        Some(&self.viewing_key)
    }

    fn uivk(&self) -> UnifiedIncomingViewingKey {
        self.viewing_key.to_unified_incoming_viewing_key()
    }
}
