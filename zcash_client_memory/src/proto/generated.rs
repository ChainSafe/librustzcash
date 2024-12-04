// This file is @generated by prost-build.
/// Unique identifier for a zcash transaction
#[derive(Clone, PartialEq, ::prost::Message)]
pub struct TxId {
    #[prost(bytes = "vec", tag = "1")]
    pub hash: ::prost::alloc::vec::Vec<u8>,
}
#[derive(Clone, PartialEq, ::prost::Message)]
pub struct Address {
    #[prost(bytes = "vec", tag = "1")]
    pub diversifier_index: ::prost::alloc::vec::Vec<u8>,
    #[prost(string, tag = "2")]
    pub address: ::prost::alloc::string::String,
}
#[derive(Clone, PartialEq, ::prost::Message)]
pub struct NoteId {
    #[prost(message, optional, tag = "1")]
    pub tx_id: ::core::option::Option<TxId>,
    #[prost(enumeration = "PoolType", tag = "2")]
    pub pool: i32,
    #[prost(uint32, tag = "3")]
    pub output_index: u32,
}
#[derive(Clone, PartialEq, ::prost::Message)]
pub struct Memo {
    #[prost(message, optional, tag = "1")]
    pub note_id: ::core::option::Option<NoteId>,
    #[prost(bytes = "vec", tag = "2")]
    pub memo: ::prost::alloc::vec::Vec<u8>,
}
#[derive(Clone, PartialEq, ::prost::Message)]
pub struct Nullifier {
    #[prost(enumeration = "ShieldedProtocol", tag = "1")]
    pub protocol: i32,
    #[prost(bytes = "vec", tag = "2")]
    pub nullifier: ::prost::alloc::vec::Vec<u8>,
}
#[derive(Clone, PartialEq, ::prost::Message)]
pub struct OutPoint {
    #[prost(bytes = "vec", tag = "1")]
    pub hash: ::prost::alloc::vec::Vec<u8>,
    #[prost(uint32, tag = "2")]
    pub n: u32,
}
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, PartialOrd, Ord, ::prost::Enumeration)]
#[repr(i32)]
pub enum PoolType {
    Transparent = 0,
    ShieldedSapling = 1,
    ShieldedOrchard = 2,
}
impl PoolType {
    /// String value of the enum field names used in the ProtoBuf definition.
    ///
    /// The values are not transformed in any way and thus are considered stable
    /// (if the ProtoBuf definition does not change) and safe for programmatic use.
    pub fn as_str_name(&self) -> &'static str {
        match self {
            Self::Transparent => "Transparent",
            Self::ShieldedSapling => "ShieldedSapling",
            Self::ShieldedOrchard => "ShieldedOrchard",
        }
    }
    /// Creates an enum from field names used in the ProtoBuf definition.
    pub fn from_str_name(value: &str) -> ::core::option::Option<Self> {
        match value {
            "Transparent" => Some(Self::Transparent),
            "ShieldedSapling" => Some(Self::ShieldedSapling),
            "ShieldedOrchard" => Some(Self::ShieldedOrchard),
            _ => None,
        }
    }
}
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, PartialOrd, Ord, ::prost::Enumeration)]
#[repr(i32)]
pub enum ShieldedProtocol {
    Sapling = 0,
    Orchard = 1,
}
impl ShieldedProtocol {
    /// String value of the enum field names used in the ProtoBuf definition.
    ///
    /// The values are not transformed in any way and thus are considered stable
    /// (if the ProtoBuf definition does not change) and safe for programmatic use.
    pub fn as_str_name(&self) -> &'static str {
        match self {
            Self::Sapling => "sapling",
            Self::Orchard => "orchard",
        }
    }
    /// Creates an enum from field names used in the ProtoBuf definition.
    pub fn from_str_name(value: &str) -> ::core::option::Option<Self> {
        match value {
            "sapling" => Some(Self::Sapling),
            "orchard" => Some(Self::Orchard),
            _ => None,
        }
    }
}
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, PartialOrd, Ord, ::prost::Enumeration)]
#[repr(i32)]
pub enum TransactionStatus {
    TxidNotRecognized = 0,
    NotInMainChain = 1,
    Mined = 2,
}
impl TransactionStatus {
    /// String value of the enum field names used in the ProtoBuf definition.
    ///
    /// The values are not transformed in any way and thus are considered stable
    /// (if the ProtoBuf definition does not change) and safe for programmatic use.
    pub fn as_str_name(&self) -> &'static str {
        match self {
            Self::TxidNotRecognized => "TxidNotRecognized",
            Self::NotInMainChain => "NotInMainChain",
            Self::Mined => "Mined",
        }
    }
    /// Creates an enum from field names used in the ProtoBuf definition.
    pub fn from_str_name(value: &str) -> ::core::option::Option<Self> {
        match value {
            "TxidNotRecognized" => Some(Self::TxidNotRecognized),
            "NotInMainChain" => Some(Self::NotInMainChain),
            "Mined" => Some(Self::Mined),
            _ => None,
        }
    }
}
#[derive(Clone, PartialEq, ::prost::Message)]
pub struct Note {
    #[prost(enumeration = "ShieldedProtocol", tag = "1")]
    pub protocol: i32,
    #[prost(bytes = "vec", tag = "2")]
    pub recipient: ::prost::alloc::vec::Vec<u8>,
    #[prost(uint64, tag = "3")]
    pub value: u64,
    #[prost(bytes = "vec", optional, tag = "4")]
    pub rho: ::core::option::Option<::prost::alloc::vec::Vec<u8>>,
    #[prost(message, optional, tag = "5")]
    pub rseed: ::core::option::Option<RSeed>,
}
#[derive(Clone, PartialEq, ::prost::Message)]
pub struct RSeed {
    #[prost(enumeration = "RSeedType", optional, tag = "1")]
    pub rseed_type: ::core::option::Option<i32>,
    #[prost(bytes = "vec", tag = "2")]
    pub payload: ::prost::alloc::vec::Vec<u8>,
}
#[derive(Clone, PartialEq, ::prost::Message)]
pub struct ReceivedNote {
    #[prost(message, optional, tag = "1")]
    pub note_id: ::core::option::Option<NoteId>,
    #[prost(message, optional, tag = "2")]
    pub tx_id: ::core::option::Option<TxId>,
    #[prost(uint32, tag = "3")]
    pub output_index: u32,
    #[prost(uint32, tag = "4")]
    pub account_id: u32,
    #[prost(message, optional, tag = "5")]
    pub note: ::core::option::Option<Note>,
    #[prost(message, optional, tag = "6")]
    pub nullifier: ::core::option::Option<Nullifier>,
    #[prost(bool, tag = "7")]
    pub is_change: bool,
    #[prost(bytes = "vec", tag = "8")]
    pub memo: ::prost::alloc::vec::Vec<u8>,
    #[prost(uint64, optional, tag = "9")]
    pub commitment_tree_position: ::core::option::Option<u64>,
    #[prost(enumeration = "Scope", optional, tag = "10")]
    pub recipient_key_scope: ::core::option::Option<i32>,
}
#[derive(Clone, PartialEq, ::prost::Message)]
pub struct SentNote {
    #[prost(uint32, tag = "1")]
    pub from_account_id: u32,
    #[prost(message, optional, tag = "2")]
    pub to: ::core::option::Option<Recipient>,
    #[prost(uint64, tag = "3")]
    pub value: u64,
    #[prost(bytes = "vec", tag = "4")]
    pub memo: ::prost::alloc::vec::Vec<u8>,
}
#[derive(Clone, PartialEq, ::prost::Message)]
pub struct Recipient {
    #[prost(enumeration = "RecipientType", tag = "1")]
    pub recipient_type: i32,
    /// either the zcash address if external or transparent address if EphemeralTransparent
    #[prost(string, optional, tag = "2")]
    pub address: ::core::option::Option<::prost::alloc::string::String>,
    /// the shielded protocol if External
    #[prost(enumeration = "PoolType", optional, tag = "3")]
    pub pool_type: ::core::option::Option<i32>,
    /// the account id if EphemeralTransparent or InternalAccount
    #[prost(uint32, optional, tag = "4")]
    pub account_id: ::core::option::Option<u32>,
    /// the outpoint metadata if InternalAccount
    #[prost(message, optional, tag = "5")]
    pub outpoint_metadata: ::core::option::Option<OutPoint>,
    /// the note if InternalAccount
    #[prost(message, optional, tag = "6")]
    pub note: ::core::option::Option<Note>,
}
/// associates a note and a transaction where it was spent
#[derive(Clone, PartialEq, ::prost::Message)]
pub struct ReceivedNoteSpendRecord {
    #[prost(message, optional, tag = "1")]
    pub note_id: ::core::option::Option<NoteId>,
    #[prost(message, optional, tag = "2")]
    pub tx_id: ::core::option::Option<TxId>,
}
/// records where a nullifier was spent by block height and tx index in that block
#[derive(Clone, PartialEq, ::prost::Message)]
pub struct NullifierRecord {
    #[prost(message, optional, tag = "1")]
    pub nullifier: ::core::option::Option<Nullifier>,
    #[prost(uint32, tag = "2")]
    pub block_height: u32,
    #[prost(uint32, tag = "3")]
    pub tx_index: u32,
}
/// Record storing the sent information for a given note
#[derive(Clone, PartialEq, ::prost::Message)]
pub struct SentNoteRecord {
    #[prost(message, optional, tag = "1")]
    pub sent_note_id: ::core::option::Option<NoteId>,
    #[prost(message, optional, tag = "2")]
    pub sent_note: ::core::option::Option<SentNote>,
}
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, PartialOrd, Ord, ::prost::Enumeration)]
#[repr(i32)]
pub enum RSeedType {
    BeforeZip212 = 0,
    AfterZip212 = 1,
}
impl RSeedType {
    /// String value of the enum field names used in the ProtoBuf definition.
    ///
    /// The values are not transformed in any way and thus are considered stable
    /// (if the ProtoBuf definition does not change) and safe for programmatic use.
    pub fn as_str_name(&self) -> &'static str {
        match self {
            Self::BeforeZip212 => "BeforeZip212",
            Self::AfterZip212 => "AfterZip212",
        }
    }
    /// Creates an enum from field names used in the ProtoBuf definition.
    pub fn from_str_name(value: &str) -> ::core::option::Option<Self> {
        match value {
            "BeforeZip212" => Some(Self::BeforeZip212),
            "AfterZip212" => Some(Self::AfterZip212),
            _ => None,
        }
    }
}
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, PartialOrd, Ord, ::prost::Enumeration)]
#[repr(i32)]
pub enum Scope {
    Internal = 0,
    External = 1,
}
impl Scope {
    /// String value of the enum field names used in the ProtoBuf definition.
    ///
    /// The values are not transformed in any way and thus are considered stable
    /// (if the ProtoBuf definition does not change) and safe for programmatic use.
    pub fn as_str_name(&self) -> &'static str {
        match self {
            Self::Internal => "Internal",
            Self::External => "External",
        }
    }
    /// Creates an enum from field names used in the ProtoBuf definition.
    pub fn from_str_name(value: &str) -> ::core::option::Option<Self> {
        match value {
            "Internal" => Some(Self::Internal),
            "External" => Some(Self::External),
            _ => None,
        }
    }
}
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, PartialOrd, Ord, ::prost::Enumeration)]
#[repr(i32)]
pub enum RecipientType {
    ExternalRecipient = 0,
    EphemeralTransparent = 1,
    InternalAccount = 2,
}
impl RecipientType {
    /// String value of the enum field names used in the ProtoBuf definition.
    ///
    /// The values are not transformed in any way and thus are considered stable
    /// (if the ProtoBuf definition does not change) and safe for programmatic use.
    pub fn as_str_name(&self) -> &'static str {
        match self {
            Self::ExternalRecipient => "ExternalRecipient",
            Self::EphemeralTransparent => "EphemeralTransparent",
            Self::InternalAccount => "InternalAccount",
        }
    }
    /// Creates an enum from field names used in the ProtoBuf definition.
    pub fn from_str_name(value: &str) -> ::core::option::Option<Self> {
        match value {
            "ExternalRecipient" => Some(Self::ExternalRecipient),
            "EphemeralTransparent" => Some(Self::EphemeralTransparent),
            "InternalAccount" => Some(Self::InternalAccount),
            _ => None,
        }
    }
}
/// A shard tree defined by a cap subtree and shard subtrees
#[derive(Clone, PartialEq, ::prost::Message)]
pub struct ShardTree {
    #[prost(bytes = "vec", tag = "1")]
    pub cap: ::prost::alloc::vec::Vec<u8>,
    #[prost(message, repeated, tag = "2")]
    pub shards: ::prost::alloc::vec::Vec<TreeShard>,
    #[prost(message, repeated, tag = "3")]
    pub checkpoints: ::prost::alloc::vec::Vec<TreeCheckpoint>,
}
/// A shard in a shard tree
#[derive(Clone, PartialEq, ::prost::Message)]
pub struct TreeShard {
    #[prost(uint64, tag = "1")]
    pub shard_index: u64,
    #[prost(bytes = "vec", tag = "3")]
    pub shard_data: ::prost::alloc::vec::Vec<u8>,
}
/// A checkpoint in a shard tree
#[derive(Clone, Copy, PartialEq, ::prost::Message)]
pub struct TreeCheckpoint {
    #[prost(uint32, tag = "1")]
    pub checkpoint_id: u32,
    #[prost(uint64, tag = "2")]
    pub position: u64,
}
/// Stores the block height corresponding to the last note commitment in a shard
/// as defined by its level and index in the tree
#[derive(Clone, Copy, PartialEq, ::prost::Message)]
pub struct TreeEndHeightsRecord {
    #[prost(uint32, tag = "1")]
    pub level: u32,
    #[prost(uint64, tag = "2")]
    pub index: u64,
    #[prost(uint32, tag = "3")]
    pub block_height: u32,
}
#[derive(Clone, PartialEq, ::prost::Message)]
pub struct ReceivedTransparentOutput {
    #[prost(bytes = "vec", tag = "1")]
    pub transaction_id: ::prost::alloc::vec::Vec<u8>,
    #[prost(uint32, tag = "2")]
    pub account_id: u32,
    #[prost(string, tag = "3")]
    pub address: ::prost::alloc::string::String,
    #[prost(message, optional, tag = "4")]
    pub txout: ::core::option::Option<TxOut>,
    #[prost(uint32, optional, tag = "5")]
    pub max_observed_unspent_height: ::core::option::Option<u32>,
}
#[derive(Clone, PartialEq, ::prost::Message)]
pub struct TxOut {
    #[prost(uint64, tag = "1")]
    pub value: u64,
    #[prost(bytes = "vec", tag = "2")]
    pub script: ::prost::alloc::vec::Vec<u8>,
}
#[derive(Clone, PartialEq, ::prost::Message)]
pub struct TransparentReceivedOutputRecord {
    #[prost(message, optional, tag = "1")]
    pub outpoint: ::core::option::Option<OutPoint>,
    #[prost(message, optional, tag = "2")]
    pub output: ::core::option::Option<ReceivedTransparentOutput>,
}
#[derive(Clone, PartialEq, ::prost::Message)]
pub struct TransparentReceivedOutputSpendRecord {
    #[prost(message, optional, tag = "1")]
    pub outpoint: ::core::option::Option<OutPoint>,
    #[prost(message, optional, tag = "2")]
    pub tx_id: ::core::option::Option<TxId>,
}
#[derive(Clone, PartialEq, ::prost::Message)]
pub struct TransparentSpendCacheRecord {
    #[prost(message, optional, tag = "1")]
    pub tx_id: ::core::option::Option<TxId>,
    #[prost(message, optional, tag = "2")]
    pub outpoint: ::core::option::Option<OutPoint>,
}
/// A serialized zcash wallet state
#[derive(Clone, PartialEq, ::prost::Message)]
pub struct MemoryWallet {
    /// the version of the wallet serialization format
    #[prost(uint32, tag = "1")]
    pub version: u32,
    /// the accounts in this wallet
    #[prost(message, optional, tag = "2")]
    pub accounts: ::core::option::Option<Accounts>,
    /// map from block height to block data
    #[prost(message, repeated, tag = "3")]
    pub blocks: ::prost::alloc::vec::Vec<WalletBlock>,
    /// map from transaction id to transaction data
    #[prost(message, repeated, tag = "4")]
    pub tx_table: ::prost::alloc::vec::Vec<TransactionTableRecord>,
    /// the notes received by this wallet
    #[prost(message, repeated, tag = "5")]
    pub received_note_table: ::prost::alloc::vec::Vec<ReceivedNote>,
    /// the notes spent by this wallet
    #[prost(message, repeated, tag = "6")]
    pub received_note_spends: ::prost::alloc::vec::Vec<ReceivedNoteSpendRecord>,
    /// the nullifiers for notes spent by this wallet
    #[prost(message, repeated, tag = "7")]
    pub nullifiers: ::prost::alloc::vec::Vec<NullifierRecord>,
    /// the notes sent by this wallet
    #[prost(message, repeated, tag = "8")]
    pub sent_notes: ::prost::alloc::vec::Vec<SentNoteRecord>,
    /// map between txIds and their inclusion in blocks
    #[prost(message, repeated, tag = "9")]
    pub tx_locator: ::prost::alloc::vec::Vec<TxLocatorRecord>,
    /// the scan queue (which blocks the wallet should scan next and with what priority)
    #[prost(message, repeated, tag = "10")]
    pub scan_queue: ::prost::alloc::vec::Vec<ScanQueueRecord>,
    /// Sapling shielded pool shard tree
    #[prost(message, optional, tag = "11")]
    pub sapling_tree: ::core::option::Option<ShardTree>,
    /// the block heights corresponding to the last note commitment for each shard in the sapling tree
    #[prost(message, repeated, tag = "12")]
    pub sapling_tree_shard_end_heights: ::prost::alloc::vec::Vec<TreeEndHeightsRecord>,
    /// Orchard shielded pool shard tree
    #[prost(message, optional, tag = "13")]
    pub orchard_tree: ::core::option::Option<ShardTree>,
    /// the block heights corresponding to the last note commitment for each shard in the orchard tree
    #[prost(message, repeated, tag = "14")]
    pub orchard_tree_shard_end_heights: ::prost::alloc::vec::Vec<TreeEndHeightsRecord>,
    /// UTXOs known to this wallet
    #[prost(message, repeated, tag = "15")]
    pub transparent_received_outputs: ::prost::alloc::vec::Vec<
        TransparentReceivedOutputRecord,
    >,
    /// UTXOs spent by this wallet
    #[prost(message, repeated, tag = "16")]
    pub transparent_received_output_spends: ::prost::alloc::vec::Vec<
        TransparentReceivedOutputSpendRecord,
    >,
    /// Map from spends to their location in the blockchain
    #[prost(message, repeated, tag = "17")]
    pub transparent_spend_map: ::prost::alloc::vec::Vec<TransparentSpendCacheRecord>,
    /// Queue of transaction data requests the wallet should make to the lightwalletd provided to obtain more complete information
    #[prost(message, repeated, tag = "18")]
    pub transaction_data_requests: ::prost::alloc::vec::Vec<TransactionDataRequest>,
}
#[derive(Clone, PartialEq, ::prost::Message)]
pub struct Accounts {
    /// map from account index to account data
    #[prost(message, repeated, tag = "1")]
    pub accounts: ::prost::alloc::vec::Vec<Account>,
    /// the nonce for the next account
    #[prost(uint32, tag = "2")]
    pub account_nonce: u32,
}
#[derive(Clone, PartialEq, ::prost::Message)]
pub struct Account {
    /// the index of this account
    #[prost(uint32, tag = "1")]
    pub account_id: u32,
    /// derived or imported
    #[prost(enumeration = "AccountKind", tag = "2")]
    pub kind: i32,
    #[prost(bytes = "vec", optional, tag = "3")]
    pub seed_fingerprint: ::core::option::Option<::prost::alloc::vec::Vec<u8>>,
    /// HD index to derive account from seed
    #[prost(uint32, optional, tag = "5")]
    pub account_index: ::core::option::Option<u32>,
    /// spending or view-only
    #[prost(enumeration = "AccountPurpose", optional, tag = "6")]
    pub purpose: ::core::option::Option<i32>,
    /// the viewing key for this account
    #[prost(string, tag = "7")]
    pub viewing_key: ::prost::alloc::string::String,
    /// the block height at which this account was created
    #[prost(message, optional, tag = "8")]
    pub birthday: ::core::option::Option<AccountBirthday>,
    /// account addresses
    #[prost(message, repeated, tag = "9")]
    pub addresses: ::prost::alloc::vec::Vec<Address>,
    /// map from index to encoded unified address
    #[prost(message, repeated, tag = "10")]
    pub ephemeral_addresses: ::prost::alloc::vec::Vec<EphemeralAddressRecord>,
    /// human readable name for the account
    #[prost(string, tag = "11")]
    pub account_name: ::prost::alloc::string::String,
}
#[derive(Clone, PartialEq, ::prost::Message)]
pub struct AccountBirthday {
    /// the chain state at the block height before the account was created
    #[prost(message, optional, tag = "1")]
    pub prior_chain_state: ::core::option::Option<ChainState>,
    /// the block height until which the account should stop being in recovery mode
    #[prost(uint32, optional, tag = "2")]
    pub recover_until: ::core::option::Option<u32>,
}
/// A record storing transaction data in the transaction table
#[derive(Clone, PartialEq, ::prost::Message)]
pub struct TransactionTableRecord {
    #[prost(message, optional, tag = "1")]
    pub tx_id: ::core::option::Option<TxId>,
    #[prost(message, optional, tag = "2")]
    pub tx_entry: ::core::option::Option<TransactionEntry>,
}
/// Maps a block height and transaction index to a transaction ID.
#[derive(Clone, PartialEq, ::prost::Message)]
pub struct TxLocatorRecord {
    #[prost(uint32, tag = "1")]
    pub block_height: u32,
    #[prost(uint32, tag = "2")]
    pub tx_index: u32,
    #[prost(message, optional, tag = "3")]
    pub tx_id: ::core::option::Option<TxId>,
}
#[derive(Clone, PartialEq, ::prost::Message)]
pub struct EphemeralAddress {
    #[prost(string, tag = "1")]
    pub address: ::prost::alloc::string::String,
    #[prost(bytes = "vec", optional, tag = "2")]
    pub used_in_tx: ::core::option::Option<::prost::alloc::vec::Vec<u8>>,
    #[prost(bytes = "vec", optional, tag = "3")]
    pub seen_in_tx: ::core::option::Option<::prost::alloc::vec::Vec<u8>>,
}
#[derive(Clone, PartialEq, ::prost::Message)]
pub struct EphemeralAddressRecord {
    #[prost(uint32, tag = "1")]
    pub index: u32,
    #[prost(message, optional, tag = "2")]
    pub ephemeral_address: ::core::option::Option<EphemeralAddress>,
}
#[derive(Clone, PartialEq, ::prost::Message)]
pub struct ChainState {
    /// the height of this block
    #[prost(uint32, tag = "1")]
    pub block_height: u32,
    #[prost(bytes = "vec", tag = "2")]
    pub block_hash: ::prost::alloc::vec::Vec<u8>,
    #[prost(bytes = "vec", tag = "3")]
    pub final_sapling_tree: ::prost::alloc::vec::Vec<u8>,
    #[prost(bytes = "vec", tag = "4")]
    pub final_orchard_tree: ::prost::alloc::vec::Vec<u8>,
}
#[derive(Clone, PartialEq, ::prost::Message)]
pub struct WalletBlock {
    /// the height of this block
    #[prost(uint32, tag = "1")]
    pub height: u32,
    /// the ID (hash) of this block, same as in block explorers
    #[prost(bytes = "vec", tag = "2")]
    pub hash: ::prost::alloc::vec::Vec<u8>,
    /// Unix epoch time when the block was mined
    #[prost(uint32, tag = "3")]
    pub block_time: u32,
    /// the txids of transactions in this block
    #[prost(bytes = "vec", repeated, tag = "4")]
    pub transactions: ::prost::alloc::vec::Vec<::prost::alloc::vec::Vec<u8>>,
    /// map from note id to memo
    #[prost(message, repeated, tag = "5")]
    pub memos: ::prost::alloc::vec::Vec<Memo>,
    /// the size of the Sapling note commitment tree as of the end of this block
    #[prost(uint32, optional, tag = "6")]
    pub sapling_commitment_tree_size: ::core::option::Option<u32>,
    /// the number of Sapling outputs in this block
    #[prost(uint32, optional, tag = "7")]
    pub sapling_output_count: ::core::option::Option<u32>,
    /// the size of the Orchard note commitment tree as of the end of this block
    #[prost(uint32, optional, tag = "8")]
    pub orchard_commitment_tree_size: ::core::option::Option<u32>,
    /// the number of Orchard actions in this block
    #[prost(uint32, optional, tag = "9")]
    pub orchard_action_count: ::core::option::Option<u32>,
}
#[derive(Clone, PartialEq, ::prost::Message)]
pub struct TransactionEntry {
    #[prost(enumeration = "TransactionStatus", tag = "1")]
    pub tx_status: i32,
    #[prost(uint32, optional, tag = "2")]
    pub block: ::core::option::Option<u32>,
    #[prost(uint32, optional, tag = "3")]
    pub tx_index: ::core::option::Option<u32>,
    #[prost(uint32, optional, tag = "4")]
    pub expiry_height: ::core::option::Option<u32>,
    #[prost(bytes = "vec", optional, tag = "5")]
    pub raw_tx: ::core::option::Option<::prost::alloc::vec::Vec<u8>>,
    #[prost(uint64, optional, tag = "6")]
    pub fee: ::core::option::Option<u64>,
    #[prost(uint32, optional, tag = "7")]
    pub target_height: ::core::option::Option<u32>,
    #[prost(uint32, optional, tag = "8")]
    pub mined_height: ::core::option::Option<u32>,
}
#[derive(Clone, PartialEq, ::prost::Message)]
pub struct TransactionDataRequest {
    #[prost(enumeration = "TransactionDataRequestType", tag = "1")]
    pub request_type: i32,
    /// for the GetStatus and Enhancement variants
    #[prost(message, optional, tag = "2")]
    pub tx_id: ::core::option::Option<TxId>,
    /// for the SpendsFromAddress variant
    #[prost(bytes = "vec", optional, tag = "3")]
    pub address: ::core::option::Option<::prost::alloc::vec::Vec<u8>>,
    #[prost(uint32, optional, tag = "4")]
    pub block_range_start: ::core::option::Option<u32>,
    #[prost(uint32, optional, tag = "5")]
    pub block_range_end: ::core::option::Option<u32>,
}
#[derive(Clone, Copy, PartialEq, ::prost::Message)]
pub struct ScanQueueRecord {
    #[prost(uint32, tag = "1")]
    pub start_height: u32,
    #[prost(uint32, tag = "2")]
    pub end_height: u32,
    #[prost(enumeration = "ScanPriority", tag = "3")]
    pub priority: i32,
}
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, PartialOrd, Ord, ::prost::Enumeration)]
#[repr(i32)]
pub enum AccountKind {
    Derived = 0,
    Imported = 1,
}
impl AccountKind {
    /// String value of the enum field names used in the ProtoBuf definition.
    ///
    /// The values are not transformed in any way and thus are considered stable
    /// (if the ProtoBuf definition does not change) and safe for programmatic use.
    pub fn as_str_name(&self) -> &'static str {
        match self {
            Self::Derived => "Derived",
            Self::Imported => "Imported",
        }
    }
    /// Creates an enum from field names used in the ProtoBuf definition.
    pub fn from_str_name(value: &str) -> ::core::option::Option<Self> {
        match value {
            "Derived" => Some(Self::Derived),
            "Imported" => Some(Self::Imported),
            _ => None,
        }
    }
}
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, PartialOrd, Ord, ::prost::Enumeration)]
#[repr(i32)]
pub enum AccountPurpose {
    Spending = 0,
    ViewOnly = 1,
}
impl AccountPurpose {
    /// String value of the enum field names used in the ProtoBuf definition.
    ///
    /// The values are not transformed in any way and thus are considered stable
    /// (if the ProtoBuf definition does not change) and safe for programmatic use.
    pub fn as_str_name(&self) -> &'static str {
        match self {
            Self::Spending => "Spending",
            Self::ViewOnly => "ViewOnly",
        }
    }
    /// Creates an enum from field names used in the ProtoBuf definition.
    pub fn from_str_name(value: &str) -> ::core::option::Option<Self> {
        match value {
            "Spending" => Some(Self::Spending),
            "ViewOnly" => Some(Self::ViewOnly),
            _ => None,
        }
    }
}
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, PartialOrd, Ord, ::prost::Enumeration)]
#[repr(i32)]
pub enum TransactionDataRequestType {
    GetStatus = 0,
    Enhancement = 1,
    SpendsFromAddress = 2,
}
impl TransactionDataRequestType {
    /// String value of the enum field names used in the ProtoBuf definition.
    ///
    /// The values are not transformed in any way and thus are considered stable
    /// (if the ProtoBuf definition does not change) and safe for programmatic use.
    pub fn as_str_name(&self) -> &'static str {
        match self {
            Self::GetStatus => "GetStatus",
            Self::Enhancement => "Enhancement",
            Self::SpendsFromAddress => "SpendsFromAddress",
        }
    }
    /// Creates an enum from field names used in the ProtoBuf definition.
    pub fn from_str_name(value: &str) -> ::core::option::Option<Self> {
        match value {
            "GetStatus" => Some(Self::GetStatus),
            "Enhancement" => Some(Self::Enhancement),
            "SpendsFromAddress" => Some(Self::SpendsFromAddress),
            _ => None,
        }
    }
}
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, PartialOrd, Ord, ::prost::Enumeration)]
#[repr(i32)]
pub enum ScanPriority {
    /// / Block ranges that are ignored have lowest priority.
    Ignored = 0,
    /// / Block ranges that have already been scanned will not be re-scanned.
    Scanned = 1,
    /// / Block ranges to be scanned to advance the fully-scanned height.
    Historic = 2,
    /// / Block ranges adjacent to heights at which the user opened the wallet.
    OpenAdjacent = 3,
    /// / Blocks that must be scanned to complete note commitment tree shards adjacent to found notes.
    FoundNote = 4,
    /// / Blocks that must be scanned to complete the latest note commitment tree shard.
    ChainTip = 5,
    /// / A previously scanned range that must be verified to check it is still in the
    /// / main chain, has highest priority.
    Verify = 6,
}
impl ScanPriority {
    /// String value of the enum field names used in the ProtoBuf definition.
    ///
    /// The values are not transformed in any way and thus are considered stable
    /// (if the ProtoBuf definition does not change) and safe for programmatic use.
    pub fn as_str_name(&self) -> &'static str {
        match self {
            Self::Ignored => "Ignored",
            Self::Scanned => "Scanned",
            Self::Historic => "Historic",
            Self::OpenAdjacent => "OpenAdjacent",
            Self::FoundNote => "FoundNote",
            Self::ChainTip => "ChainTip",
            Self::Verify => "Verify",
        }
    }
    /// Creates an enum from field names used in the ProtoBuf definition.
    pub fn from_str_name(value: &str) -> ::core::option::Option<Self> {
        match value {
            "Ignored" => Some(Self::Ignored),
            "Scanned" => Some(Self::Scanned),
            "Historic" => Some(Self::Historic),
            "OpenAdjacent" => Some(Self::OpenAdjacent),
            "FoundNote" => Some(Self::FoundNote),
            "ChainTip" => Some(Self::ChainTip),
            "Verify" => Some(Self::Verify),
            _ => None,
        }
    }
}
