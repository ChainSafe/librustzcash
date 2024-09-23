use std::convert::Infallible;

use shardtree::error::ShardTreeError;
use zcash_keys::keys::{AddressGenerationError, DerivationError};
use zcash_primitives::{legacy::TransparentAddress, transaction::TxId};
use zcash_protocol::{consensus::BlockHeight, memo};

use crate::AccountId;

type Type = AddressGenerationError;

#[derive(Debug, thiserror::Error)]
pub enum Error {
    #[error("Account not found: {0:?}")]
    AccountUnknown(AccountId),
    #[error("Viewing key not found for account: {0:?}")]
    ViewingKeyNotFound(AccountId),
    #[error("Memo decryption failed: {0}")]
    MemoDecryption(memo::Error),
    #[error("Error deriving key: {0}")]
    KeyDerivation(DerivationError),
    #[error("Unknown ZIP32 derivation")]
    UnknownZip32Derivation,
    #[error("Error generating address: {0}")]
    AddressGeneration(Type),
    #[error("Seed must be between 32 and 252 bytes in length.")]
    InvalidSeedLength,
    #[error("Account out of range.")]
    AccountOutOfRange,
    #[error("Transaction not in table: {0}")]
    TransactionNotFound(TxId),
    #[error("Note not found")]
    NoteNotFound,
    #[error("Conflicting Tx Locator map entry")]
    ConflictingTxLocator,
    #[error("Io Error: {0}")]
    Io(std::io::Error),
    #[error("Corrupted Data: {0}")]
    CorruptedData(String),
    #[error("An error occurred while processing an account due to a failure in deriving the account's keys: {0}")]
    BadAccountData(String),
    #[error("Blocks are non sequental")]
    NonSequentialBlocks,
    #[error("Invalid scan range start {0}, end {1}: {2}")]
    InvalidScanRange(BlockHeight, BlockHeight, String),
    #[error("ShardTree error: {0}")]
    ShardTree(ShardTreeError<Infallible>),
    #[error("Balance error: {0}")]
    Balance(#[from] zcash_protocol::value::BalanceError),
    #[error("Other error: {0}")]
    Other(String),
    #[error("Infallible")]
    Infallible(#[from] Infallible),
    #[error("Expected field missing: {0}")]
    Missing(String),
    #[error("Orchard specific code was called without the 'orchard' feature enabled")]
    OrchardNotEnabled,
    #[error("Address not recognized: {0:?}")]
    AddressNotRecognized(TransparentAddress),
    #[error("Requested rewind to invalid block height. Safe height: {0:?}, requested height {1:?}")]
    RequestedRewindInvalid(BlockHeight, BlockHeight),
    #[cfg(feature = "transparent-inputs")]
    #[error("Requested gap limit {1} reached for account {0:?}")]
    ReachedGapLimit(AccountId, u32),
}

impl From<DerivationError> for Error {
    fn from(value: DerivationError) -> Self {
        Error::KeyDerivation(value)
    }
}

impl From<AddressGenerationError> for Error {
    fn from(value: AddressGenerationError) -> Self {
        Error::AddressGeneration(value)
    }
}

impl From<memo::Error> for Error {
    fn from(value: memo::Error) -> Self {
        Error::MemoDecryption(value)
    }
}

impl From<std::io::Error> for Error {
    fn from(value: std::io::Error) -> Self {
        Error::Io(value)
    }
}

impl From<ShardTreeError<Infallible>> for Error {
    fn from(value: ShardTreeError<Infallible>) -> Self {
        Error::ShardTree(value)
    }
}
