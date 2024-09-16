use serde::Deserializer;
use serde::{Deserialize, Serialize};

use serde_with::DeserializeAs;
use serde_with::SerializeAs;
use zcash_protocol::memo::Memo;
use zcash_protocol::memo::MemoBytes;

pub struct MemoBytesDef;
impl SerializeAs<MemoBytes> for MemoBytesDef {
    fn serialize_as<S>(value: &MemoBytes, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: serde::Serializer,
    {
        value.as_slice().serialize(serializer)
    }
}

impl<'de> DeserializeAs<'de, MemoBytes> for MemoBytesDef {
    fn deserialize_as<D>(deserializer: D) -> Result<MemoBytes, D::Error>
    where
        D: Deserializer<'de>,
    {
        let b = <Vec<u8>>::deserialize(deserializer)?;
        MemoBytes::from_bytes(&b).map_err(|_| serde::de::Error::custom("Invalid memo bytes"))
    }
}

impl SerializeAs<Memo> for MemoBytesDef {
    fn serialize_as<S>(value: &Memo, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: serde::Serializer,
    {
        value.encode().as_slice().serialize(serializer)
    }
}

impl<'de> DeserializeAs<'de, Memo> for MemoBytesDef {
    fn deserialize_as<D>(deserializer: D) -> Result<Memo, D::Error>
    where
        D: Deserializer<'de>,
    {
        let b = <Vec<u8>>::deserialize(deserializer)?;
        Memo::from_bytes(&b).map_err(|_| serde::de::Error::custom("Invalid memo"))
    }
}
