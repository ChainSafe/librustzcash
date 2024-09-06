use serde::{Deserialize, Serialize};

use zcash_protocol::memo::Memo;
use zcash_protocol::memo::MemoBytes;

pub struct MemoBytesWrapper;
impl serde_with::SerializeAs<MemoBytes> for MemoBytesWrapper {
    fn serialize_as<S>(value: &MemoBytes, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: serde::Serializer,
    {
        value.as_slice().serialize(serializer)
    }
}

impl<'de> serde_with::DeserializeAs<'de, MemoBytes> for MemoBytesWrapper {
    fn deserialize_as<D>(deserializer: D) -> Result<MemoBytes, D::Error>
    where
        D: serde::Deserializer<'de>,
    {
        let b = <Vec<u8>>::deserialize(deserializer)?;
        MemoBytes::from_bytes(&b).map_err(|_| serde::de::Error::custom("Invalid memo bytes"))
    }
}

impl serde_with::SerializeAs<Memo> for MemoBytesWrapper {
    fn serialize_as<S>(value: &Memo, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: serde::Serializer,
    {
        value.encode().as_slice().serialize(serializer)
    }
}

impl<'de> serde_with::DeserializeAs<'de, Memo> for MemoBytesWrapper {
    fn deserialize_as<D>(deserializer: D) -> Result<Memo, D::Error>
    where
        D: serde::Deserializer<'de>,
    {
        let b = <Vec<u8>>::deserialize(deserializer)?;
        Memo::from_bytes(&b).map_err(|_| serde::de::Error::custom("Invalid memo"))
    }
}
