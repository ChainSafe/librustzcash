use serde::{Deserialize, Deserializer, Serialize, Serializer};

use serde_with::{DeserializeAs, SerializeAs};

use zcash_client_backend::data_api::scanning::ScanPriority;

#[derive(Serialize, Deserialize)]
#[serde(remote = "zcash_client_backend::data_api::scanning::ScanPriority")]
pub enum ScanPriorityDef {
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
impl SerializeAs<ScanPriority> for ScanPriorityDef {
    fn serialize_as<S>(value: &ScanPriority, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: Serializer,
    {
        ScanPriorityDef::serialize(value, serializer)
    }
}
impl<'de> DeserializeAs<'de, ScanPriority> for ScanPriorityDef {
    fn deserialize_as<D>(deserializer: D) -> Result<ScanPriority, D::Error>
    where
        D: Deserializer<'de>,
    {
        ScanPriorityDef::deserialize(deserializer).map(Into::into)
    }
}
