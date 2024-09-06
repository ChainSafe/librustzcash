








use serde::{Deserialize, Serialize};


use serde_with::{DeserializeAs};
use serde_with::{SerializeAs};



use zcash_client_backend::data_api::scanning::ScanPriority;










#[derive(Serialize, Deserialize)]
#[serde(remote = "zcash_client_backend::data_api::scanning::ScanPriority")]
pub enum ScanPriorityWrapper {
    /// Block ranges that are ignored have lowest priority.
    Ignored,
    /// Block ranges that have already been scanned will not be re-scanned.
    Scanned,
    /// Block ranges to be scanned to advance the fully-scanned height.
    Historic,
    /// Block ranges adjacent to heights at which the user opened the wallet.
    OpenAdjacent,
    /// Blocks that must be scanned to complete note commitment tree shards adjacent to found notes.
    FoundNote,
    /// Blocks that must be scanned to complete the latest note commitment tree shard.
    ChainTip,
    /// A previously scanned range that must be verified to check it is still in the
    /// main chain, has highest priority.
    Verify,
}
impl SerializeAs<ScanPriority> for ScanPriorityWrapper {
    fn serialize_as<S>(value: &ScanPriority, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: serde::Serializer,
    {
        ScanPriorityWrapper::serialize(value, serializer)
    }
}
impl<'de> DeserializeAs<'de, ScanPriority> for ScanPriorityWrapper {
    fn deserialize_as<D>(deserializer: D) -> Result<ScanPriority, D::Error>
    where
        D: serde::Deserializer<'de>,
    {
        ScanPriorityWrapper::deserialize(deserializer).map(Into::into)
    }
}
