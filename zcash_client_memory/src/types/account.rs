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

use zcash_address::ZcashAddress;
use zcash_client_backend::data_api::{AccountBirthday, GAP_LIMIT};
use zcash_client_backend::{
    address::UnifiedAddress,
    data_api::{Account as _, AccountPurpose, AccountSource},
    keys::{UnifiedAddressRequest, UnifiedFullViewingKey},
    wallet::NoteId,
};
use zcash_keys::address::Receiver;
use zcash_primitives::legacy::TransparentAddress;
use zcash_primitives::transaction::TxId;
use zcash_protocol::consensus::NetworkType;
#[cfg(feature = "transparent-inputs")]
use {
    zcash_client_backend::wallet::TransparentAddressMetadata,
    zcash_primitives::legacy::keys::{
        AccountPubKey, EphemeralIvk, IncomingViewingKey, NonHardenedChildIndex, TransparentKeyScope,
    },
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
        self.nonce += 1;
        let account_id = AccountId(self.nonce);

        let acc = Account::new(account_id, kind, viewing_key, birthday, purpose)?;

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

    #[cfg(feature = "transparent-inputs")]
    pub(crate) fn find_account_for_transparent_address(
        &self,
        address: &TransparentAddress,
    ) -> Result<Option<AccountId>, Error> {
        // Look for transparent receivers generated as part of a Unified Address
        if let Some(id) = self
            .accounts
            .iter()
            .find(|(_, account)| {
                account
                    .addresses()
                    .iter()
                    .any(|(_, unified_address)| unified_address.transparent() == Some(address))
            })
            .map(|(id, _)| *id)
        {
            Ok(Some(id))
        } else {
            // then look at ephemeral addresses
            if let Some(id) = self.find_account_for_ephemeral_address(address)? {
                Ok(Some(id))
            } else {
                for (account_id, account) in self.accounts.iter() {
                    if account.get_legacy_transparent_address()?.is_some() {
                        return Ok(Some(*account_id));
                    }
                }
                Ok(None)
            }
        }
    }

    #[cfg(feature = "transparent-inputs")]
    pub(crate) fn find_account_for_ephemeral_address(
        &self,
        address: &TransparentAddress,
    ) -> Result<Option<AccountId>, Error> {
        for (account_id, account) in self.accounts.iter() {
            let contains = account
                .ephemeral_addresses()?
                .iter()
                .any(|(eph_addr, _)| eph_addr == address);
            if contains {
                return Ok(Some(*account_id));
            }
        }
        Ok(None)
    }

    #[cfg(feature = "transparent-inputs")]
    pub(crate) fn mark_ephemeral_address_as_seen(
        &mut self,
        address: &TransparentAddress,
        tx_id: TxId,
    ) -> Result<(), Error> {
        for (_, account) in self.accounts.iter_mut() {
            account.mark_ephemeral_address_as_seen(address, tx_id)?
        }
        Ok(())
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

#[derive(Debug, Clone, Serialize, Deserialize)]
pub(crate) struct EphemeralAddress {
    pub(crate) address: TransparentAddress,
    // Used implies seen
    pub(crate) used: Option<TxId>,
    pub(crate) seen: Option<TxId>,
}

impl EphemeralAddress {
    fn mark_used(&mut self, tx: TxId) {
        // We update both `used_in_tx` and `seen_in_tx` here, because a used address has
        // necessarily been seen in a transaction. We will not treat this as extending the
        // range of addresses that are safe to reserve unless and until the transaction is
        // observed as mined.
        self.used.replace(tx);
        self.seen.replace(tx);
    }
    fn mark_seen(&mut self, tx: TxId) -> Option<TxId> {
        self.seen.replace(tx)
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
    #[serde_as(
        as = "BTreeMap<serde_with::FromInto<DiversifierIndexDef>, serde_with::FromInto<UnifiedAddressDef>>"
    )]
    addresses: BTreeMap<DiversifierIndex, UnifiedAddress>,

    #[cfg(feature = "transparent-inputs")]
    pub(crate) ephemeral_addresses: BTreeMap<u32, EphemeralAddress>, // NonHardenedChildIndex (< 1 << 31)

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
            #[cfg(feature = "transparent-inputs")]
            ephemeral_addresses: BTreeMap::new(),
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
        #[cfg(feature = "transparent-inputs")]
        acc.reserve_until(0)?;
        Ok(acc)
    }

    pub fn addresses(&self) -> &BTreeMap<DiversifierIndex, UnifiedAddress> {
        &self.addresses
    }

    pub fn select_receiving_address(
        &self,
        network: NetworkType,
        receiver: &Receiver,
    ) -> Result<Option<ZcashAddress>, Error> {
        Ok(self
            .addresses
            .values()
            .map(|ua| ua.to_address(network))
            .find(|addr| receiver.corresponds(addr)))
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

    #[cfg(feature = "transparent-inputs")]
    pub(crate) fn get_legacy_transparent_address(
        &self,
    ) -> Result<Option<(TransparentAddress, NonHardenedChildIndex)>, Error> {
        Ok(self
            .uivk()
            .transparent()
            .as_ref()
            .map(|tivk| tivk.default_address()))
    }
}
#[cfg(feature = "transparent-inputs")]
impl Account {
    pub fn ephemeral_addresses(
        &self,
    ) -> Result<Vec<(TransparentAddress, TransparentAddressMetadata)>, Error> {
        Ok(self
            .ephemeral_addresses
            .iter()
            .map(|(idx, addr)| {
                (
                    addr.address,
                    TransparentAddressMetadata::new(
                        TransparentKeyScope::EPHEMERAL,
                        NonHardenedChildIndex::from_index(*idx).unwrap(),
                    ),
                )
            })
            .collect())
    }
    pub fn ephemeral_ivk(&self) -> Result<Option<EphemeralIvk>, Error> {
        self.viewing_key
            .transparent()
            .map(AccountPubKey::derive_ephemeral_ivk)
            .transpose()
            .map_err(Into::into)
    }

    pub fn first_unstored_index(&self) -> Result<u32, Error> {
        if let Some((idx, _)) = self.ephemeral_addresses.last_key_value() {
            if *idx >= (1 << 31) + GAP_LIMIT {
                unreachable!("violates constraint index_range_and_address_nullity")
            } else {
                Ok(idx.checked_add(1).unwrap())
            }
        } else {
            Ok(0)
        }
    }

    pub fn first_unreserved_index(&self) -> Result<u32, Error> {
        self.first_unstored_index()?
            .checked_sub(GAP_LIMIT)
            .ok_or(Error::CorruptedData(
                "ephemeral_addresses corrupted".to_owned(),
            ))
    }

    pub fn reserve_until(
        &mut self,
        next_to_reserve: u32,
    ) -> Result<Vec<(TransparentAddress, TransparentAddressMetadata)>, Error> {
        if let Some(ephemeral_ivk) = self.ephemeral_ivk()? {
            let first_unstored = self.first_unstored_index()?;
            let range_to_store = first_unstored..(next_to_reserve.checked_add(GAP_LIMIT).unwrap());
            if range_to_store.is_empty() {
                return Ok(Vec::new());
            }
            return range_to_store
                .map(|raw_index| {
                    NonHardenedChildIndex::from_index(raw_index)
                        .map(|address_index| {
                            ephemeral_ivk
                                .derive_ephemeral_address(address_index)
                                .map(|addr| {
                                    self.ephemeral_addresses.insert(
                                        raw_index,
                                        EphemeralAddress {
                                            address: addr,
                                            seen: None,
                                            used: None,
                                        },
                                    );
                                    (
                                        addr,
                                        TransparentAddressMetadata::new(
                                            TransparentKeyScope::EPHEMERAL,
                                            address_index,
                                        ),
                                    )
                                })
                        })
                        .unwrap()
                        .map_err(Into::into)
                })
                .collect::<Result<Vec<_>, _>>();
        }
        Ok(Vec::new())
    }

    #[cfg(feature = "transparent-inputs")]
    pub fn mark_ephemeral_address_as_used(
        &mut self,
        address: &TransparentAddress,
        tx_id: TxId,
    ) -> Result<(), Error> {
        // TODO: ephemeral_address_reuse_check
        for (idx, addr) in self.ephemeral_addresses.iter_mut() {
            if addr.address == *address {
                addr.mark_used(tx_id);

                // Maintain the invariant that the last `GAP_LIMIT` addresses are used and unseen.
                let next_to_reserve = idx.checked_add(1).expect("ensured by constraint");
                self.reserve_until(next_to_reserve)?;
                return Ok(());
            }
        }
        Ok(())
    }

    #[cfg(feature = "transparent-inputs")]
    pub fn mark_ephemeral_address_as_seen(
        &mut self,
        // txns: &TransactionTable,
        address: &TransparentAddress,
        tx_id: TxId,
    ) -> Result<(), Error> {
        for (idx, addr) in self.ephemeral_addresses.iter_mut() {
            if addr.address == *address {
                // TODO: this
                // Figure out which transaction was mined earlier: `tx_ref`, or any existing
                // tx referenced by `seen_in_tx` for the given address. Prefer the existing
                // reference in case of a tie or if both transactions are unmined.
                // This slightly reduces the chance of unnecessarily reaching the gap limit
                // too early in some corner cases (because the earlier transaction is less
                // likely to be unmined).
                //
                // The query should always return a value if `tx_ref` is valid.

                addr.mark_seen(tx_id);
                // Maintain the invariant that the last `GAP_LIMIT` addresses are used and unseen.
                let next_to_reserve = idx.checked_add(1).expect("ensured by constraint");
                self.reserve_until(next_to_reserve)?;
                return Ok(());
            }
        }
        Ok(())
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
