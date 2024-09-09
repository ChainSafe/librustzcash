use serde::{Deserialize, Serialize};

use zcash_primitives::block::BlockHash;

mod shardtree;
pub use shardtree::*;
mod notes;
pub use notes::*;
mod account;
pub use account::*;
mod transaction;
pub use transaction::*;
mod scanning;
pub use scanning::*;
mod nullifier;

mod memo;
pub use memo::*;

pub use array::*;
pub use bytes::*;

pub(crate) struct BlockHashWrapper;
impl serde_with::SerializeAs<BlockHash> for BlockHashWrapper {
    fn serialize_as<S>(value: &BlockHash, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: serde::Serializer,
    {
        value.0.serialize(serializer)
    }
}
impl<'de> serde_with::DeserializeAs<'de, BlockHash> for BlockHashWrapper {
    fn deserialize_as<D>(deserializer: D) -> Result<BlockHash, D::Error>
    where
        D: serde::Deserializer<'de>,
    {
        Ok(BlockHash(<[u8; 32]>::deserialize(deserializer)?))
    }
}

mod array {
    use std::{fmt::Display, sync::Arc};
    pub trait ToArray<T, const N: usize> {
        fn to_array(&self) -> [T; N];
    }
    impl<T: ToArray<U, N>, U, const N: usize> ToArray<U, N> for Arc<T> {
        fn to_array(&self) -> [U; N] {
            self.as_ref().to_array()
        }
    }
    impl<T: TryFromArray<U, N>, U, const N: usize> TryFromArray<U, N> for Arc<T> {
        type Error = T::Error;
        fn try_from_array(arr: [U; N]) -> Result<Self, Self::Error> {
            Ok(Arc::new(T::try_from_array(arr)?))
        }
    }
    pub trait FromArray<T, const N: usize> {
        fn from_array(arr: [T; N]) -> Self;
    }

    pub trait TryFromArray<T, const N: usize>
    where
        Self: Sized,
    {
        type Error: Display;
        fn try_from_array(arr: [T; N]) -> Result<Self, Self::Error>;
    }
}

mod bytes {
    use serde_with::{DeserializeAs, SerializeAs};
    use std::io;
    pub trait ToFromBytes {
        /// Serializes this node into a byte vector.
        fn to_bytes(&self) -> Vec<u8>;

        /// Parses a node from a byte vector.
        fn from_bytes(bytes: &[u8]) -> io::Result<Self>
        where
            Self: Sized;
    }
    pub struct ToFromBytesWrapper<T: ToFromBytes>(T);

    impl<T: ToFromBytes> SerializeAs<T> for ToFromBytesWrapper<T> {
        fn serialize_as<S>(value: &T, serializer: S) -> Result<S::Ok, S::Error>
        where
            S: serde::Serializer,
        {
            value.to_bytes().serialize(serializer)
        }
    }
    impl<T: ToFromBytes> SerializeAs<&T> for ToFromBytesWrapper<T> {
        fn serialize_as<S>(value: &&T, serializer: S) -> Result<S::Ok, S::Error>
        where
            S: serde::Serializer,
        {
            value.to_bytes().serialize(serializer)
        }
    }
    impl<'de, T: ToFromBytes> DeserializeAs<'de, T> for ToFromBytesWrapper<T> {
        fn deserialize_as<D>(deserializer: D) -> Result<T, D::Error>
        where
            D: serde::Deserializer<'de>,
        {
            T::from_bytes(<Vec<u8>>::deserialize(deserializer)?.as_slice())
                .map_err(serde::de::Error::custom)
        }
    }
    impl<T: ToFromBytes> Serialize for ToFromBytesWrapper<T> {
        fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
        where
            S: serde::Serializer,
        {
            ToFromBytesWrapper::<T>::serialize_as(&self.0, serializer)
        }
    }
    impl<'de, T: ToFromBytes> Deserialize<'de> for ToFromBytesWrapper<T> {
        fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
        where
            D: serde::Deserializer<'de>,
        {
            ToFromBytesWrapper::<T>::deserialize_as(deserializer).map(ToFromBytesWrapper)
        }
    }
}
