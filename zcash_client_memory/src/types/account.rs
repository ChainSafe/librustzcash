use incrementalmerkletree::frontier::Frontier;
use serde::{Deserialize, Serialize};
use serde_with::serde_as;
use std::{
    collections::{BTreeMap, BTreeSet, HashSet},
    ops::{Deref, DerefMut},
};
use subtle::ConditionallySelectable;
use zcash_keys::keys::{AddressGenerationError, UnifiedIncomingViewingKey};
use zip32::DiversifierIndex;

use crate::error::Error;
use crate::serialization::*;
use serde_with::Seq;
use serde_with::SetPreventDuplicates;
use serde_with::TryFromInto;
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
        viewing_key: ViewingKey,
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
            .uivk()
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
    #[serde(skip_serializing)]
    viewing_key: ViewingKey,
    #[serde_as(as = "serialization::AccountBirthdayWrapper")]
    birthday: AccountBirthday,
    #[serde_as(as = "AccountPurposeWrapper")]
    _purpose: AccountPurpose, // TODO: Remove this. AccountSource should be sufficient.
    #[serde(skip_serializing)]
    addresses: BTreeMap<DiversifierIndex, UnifiedAddress>,
    #[serde_as(as = "BTreeSet<NoteIdWrapper>")]
    _notes: BTreeSet<NoteId>,
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
    _Incoming(Box<UnifiedIncomingViewingKey>),
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
    pub(crate) fn _viewing_key(&self) -> &ViewingKey {
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
            ViewingKey::_Incoming(_) => None,
        }
    }

    fn uivk(&self) -> UnifiedIncomingViewingKey {
        match self {
            ViewingKey::Full(ufvk) => ufvk.as_ref().to_unified_incoming_viewing_key(),
            ViewingKey::_Incoming(uivk) => uivk.as_ref().clone(),
        }
    }
}

mod serialization {
    use crate::types::serialization::*;
    use incrementalmerkletree::{
        frontier::{Frontier, FrontierError, NonEmptyFrontier},
        Position,
    };
    use jubjub::Fr;
    use serde::{ser::SerializeStruct, Deserialize, Serialize, Serializer};
    use serde_with::{
        ser::SerializeAsWrap, serde_as, DeserializeAs, FromInto, Seq, SerializeAs, TryFromInto,
    };
    use zcash_client_backend::data_api::{chain::ChainState, AccountBirthday};
    use zcash_primitives::{block::BlockHash, merkle_tree::HashSer};
    use zcash_protocol::consensus::BlockHeight;

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
                wrapper.prior_chain_state.into(),
                wrapper.recover_until.map(Into::into),
            )
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
        #[serde_as(as = "TryFromInto<SaplingFrontierWrapper>")]
        #[serde(getter = "zcash_client_backend::data_api::chain::ChainState::final_sapling_tree")]
        pub final_sapling_tree: Frontier<sapling::Node, { sapling::NOTE_COMMITMENT_TREE_DEPTH }>,
        #[cfg(feature = "orchard")]
        #[serde_as(as = "TryFromInto<OrchardFrontierWrapper>")]
        #[serde(getter = "zcash_client_backend::data_api::chain::ChainState::final_orchard_tree")]
        pub final_orchard_tree: Frontier<
            orchard::tree::MerkleHashOrchard,
            { orchard::NOTE_COMMITMENT_TREE_DEPTH as u8 },
        >,
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

    #[derive(Serialize, Deserialize)]
    pub struct SaplingFrontierWrapper {
        pub frontier: Option<NonEmptySaplingFrontierWrapper>,
    }

    #[cfg(feature = "orchard")]
    #[derive(Serialize, Deserialize)]
    pub struct OrchardFrontierWrapper {
        pub frontier: Option<NonEmptyOrchardFrontierWrapper>,
    }

    type NonEmptyFrontierSapling = NonEmptyFrontier<sapling::Node>;
    #[cfg(feature = "orchard")]
    type NonEmptyFrontierOrchard = NonEmptyFrontier<orchard::tree::MerkleHashOrchard>;

    #[serde_as]
    #[derive(Serialize, Deserialize)]
    pub struct NonEmptySaplingFrontierWrapper {
        #[serde_as(as = "FromInto<u64>")]
        pub position: Position,
        #[serde_as(as = "SaplingNodeWrapper")]
        pub leaf: sapling::Node,
        #[serde_as(as = "Vec<SaplingNodeWrapper>")]
        pub ommers: Vec<sapling::Node>,
    }

    impl From<Frontier<sapling::Node, { sapling::NOTE_COMMITMENT_TREE_DEPTH }>>
        for SaplingFrontierWrapper
    {
        fn from(
            frontier: Frontier<sapling::Node, { sapling::NOTE_COMMITMENT_TREE_DEPTH }>,
        ) -> Self {
            match frontier.take().and_then(|f| Some(f.into_parts())) {
                Some((position, leaf, ommers)) => SaplingFrontierWrapper {
                    frontier: Some(NonEmptySaplingFrontierWrapper {
                        position,
                        leaf,
                        ommers,
                    }),
                },
                None => SaplingFrontierWrapper { frontier: None },
            }
        }
    }

    impl TryInto<Frontier<sapling::Node, { sapling::NOTE_COMMITMENT_TREE_DEPTH }>>
        for SaplingFrontierWrapper
    {
        type Error = String;
        fn try_into(
            self,
        ) -> Result<Frontier<sapling::Node, { sapling::NOTE_COMMITMENT_TREE_DEPTH }>, Self::Error>
        {
            match self.frontier {
                Some(n) => {
                    let NonEmptySaplingFrontierWrapper {
                        position,
                        leaf,
                        ommers,
                    } = n;
                    Frontier::from_parts(position, leaf, ommers).map_err(|e| format!("{:?}", e))
                }
                None => Ok(Frontier::empty()),
            }
        }
    }
    #[cfg(feature = "orchard")]
    impl
        From<
            Frontier<
                orchard::tree::MerkleHashOrchard,
                { orchard::NOTE_COMMITMENT_TREE_DEPTH as u8 },
            >,
        > for OrchardFrontierWrapper
    {
        fn from(
            frontier: Frontier<
                orchard::tree::MerkleHashOrchard,
                { orchard::NOTE_COMMITMENT_TREE_DEPTH as u8 },
            >,
        ) -> Self {
            match frontier.take().and_then(|f| Some(f.into_parts())) {
                Some((position, leaf, ommers)) => OrchardFrontierWrapper {
                    frontier: Some(NonEmptyOrchardFrontierWrapper {
                        position,
                        leaf,
                        ommers,
                    }),
                },
                None => OrchardFrontierWrapper { frontier: None },
            }
        }
    }
    #[cfg(feature = "orchard")]
    impl
        TryInto<
            Frontier<
                orchard::tree::MerkleHashOrchard,
                { orchard::NOTE_COMMITMENT_TREE_DEPTH as u8 },
            >,
        > for OrchardFrontierWrapper
    {
        type Error = String;
        fn try_into(
            self,
        ) -> Result<
            Frontier<
                orchard::tree::MerkleHashOrchard,
                { orchard::NOTE_COMMITMENT_TREE_DEPTH as u8 },
            >,
            Self::Error,
        > {
            match self.frontier {
                Some(n) => {
                    let NonEmptyOrchardFrontierWrapper {
                        position,
                        leaf,
                        ommers,
                    } = n;
                    Frontier::from_parts(position, leaf, ommers).map_err(|e| format!("{:?}", e))
                }
                None => Ok(Frontier::empty()),
            }
        }
    }

    pub(crate) struct SaplingNodeWrapper;
    impl SerializeAs<sapling::Node> for SaplingNodeWrapper {
        fn serialize_as<S>(source: &sapling::Node, serializer: S) -> Result<S::Ok, S::Error>
        where
            S: Serializer,
        {
            source.to_bytes().serialize(serializer)
        }
    }
    impl<'de> DeserializeAs<'de, sapling::Node> for SaplingNodeWrapper {
        fn deserialize_as<D>(deserializer: D) -> Result<sapling::Node, D::Error>
        where
            D: serde::Deserializer<'de>,
        {
            let bytes = <[u8; 32]>::deserialize(deserializer)?;
            sapling::Node::from_bytes(bytes)
                .into_option()
                .ok_or_else(|| serde::de::Error::custom("Invalid sapling node "))
        }
    }

    #[cfg(feature = "orchard")]
    #[serde_as]
    #[derive(Serialize, Deserialize)]
    pub struct NonEmptyOrchardFrontierWrapper {
        #[serde_as(as = "FromInto<u64>")]
        pub position: Position,
        #[serde_as(as = "OrchardNodeWrapper")]
        pub leaf: orchard::tree::MerkleHashOrchard,
        #[serde_as(as = "Vec<OrchardNodeWrapper>")]
        pub ommers: Vec<orchard::tree::MerkleHashOrchard>,
    }

    #[cfg(feature = "orchard")]
    pub(crate) struct OrchardNodeWrapper;
    #[cfg(feature = "orchard")]
    impl SerializeAs<orchard::tree::MerkleHashOrchard> for OrchardNodeWrapper {
        fn serialize_as<S>(
            source: &orchard::tree::MerkleHashOrchard,
            serializer: S,
        ) -> Result<S::Ok, S::Error>
        where
            S: Serializer,
        {
            source.to_bytes().serialize(serializer)
        }
    }
    #[cfg(feature = "orchard")]
    impl<'de> DeserializeAs<'de, orchard::tree::MerkleHashOrchard> for OrchardNodeWrapper {
        fn deserialize_as<D>(deserializer: D) -> Result<orchard::tree::MerkleHashOrchard, D::Error>
        where
            D: serde::Deserializer<'de>,
        {
            let bytes = <[u8; 32]>::deserialize(deserializer)?;
            orchard::tree::MerkleHashOrchard::from_bytes(&bytes)
                .into_option()
                .ok_or_else(|| serde::de::Error::custom("Invalid orchard node "))
        }
    }
}
