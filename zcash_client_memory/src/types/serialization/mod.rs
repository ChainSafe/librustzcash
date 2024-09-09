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

impl ToArray<u8, 32> for BlockHash {
    fn to_array(&self) -> [u8; 32] {
        self.0
    }
}
impl FromArray<u8, 32> for BlockHash {
    fn from_array(arr: [u8; 32]) -> Self {
        BlockHash(arr)
    }
}
mod array {
    use serde::{Deserialize, Serialize};
    use serde_with::{serde_as, SerializeAs};
    use serde_with::{Bytes, DeserializeAs};
    use std::convert::Infallible;
    use std::{fmt::Display, sync::Arc};

    /// A trait for converting a type to an array.
    pub trait ToArray<T, const N: usize> {
        fn to_array(&self) -> [T; N];
    }

    /// A trait for converting an array to a type.
    pub trait FromArray<T, const N: usize> {
        fn from_array(arr: [T; N]) -> Self;
    }

    /// A trait for converting an array to a type, with the possibility of failure.
    pub trait TryFromArray<T, const N: usize>
    where
        Self: Sized,
    {
        type Error: Display;
        fn try_from_array(arr: [T; N]) -> Result<Self, Self::Error>;
    }

    pub trait TryToArray<T, const N: usize>
    where
        Self: Sized,
    {
        type Error: Display;
        fn try_to_array(&self) -> Result<[T; N], Self::Error>;
    }

    impl<T: ToArray<U, N>, U, const N: usize> ToArray<U, N> for Arc<T> {
        fn to_array(&self) -> [U; N] {
            self.as_ref().to_array()
        }
    }

    /// Blanket impl: Everything that can be infallibly converted from an array should also get Try for free
    impl<U: FromArray<T, N>, T, const N: usize> TryFromArray<T, N> for U {
        type Error = Infallible;
        fn try_from_array(arr: [T; N]) -> Result<Self, Self::Error> {
            Ok(Self::from_array(arr))
        }
    }
    /// Blanket impl: Everything that can be infallibly converted to an array should also get Try for free
    impl<U: ToArray<T, N>, T, const N: usize> TryToArray<T, N> for U {
        type Error = Infallible;
        fn try_to_array(&self) -> Result<[T; N], Self::Error> {
            Ok(self.to_array())
        }
    }

    #[serde_as]
    #[derive(Serialize, Deserialize)]
    /// A wrapper for serializing and deserializing arrays as fixed byte sequences.
    pub struct ByteArray<const N: usize>(#[serde_as(as = "Bytes")] [u8; N]);
    impl<T: ToArray<u8, N>, const N: usize> SerializeAs<T> for ByteArray<N> {
        fn serialize_as<S>(value: &T, serializer: S) -> Result<S::Ok, S::Error>
        where
            S: serde::Serializer,
        {
            ByteArray(value.to_array()).serialize(serializer)
        }
    }
    impl<'de, T: FromArray<u8, N>, const N: usize> DeserializeAs<'de, T> for ByteArray<N> {
        fn deserialize_as<D>(deserializer: D) -> Result<T, D::Error>
        where
            D: serde::Deserializer<'de>,
        {
            Ok(T::from_array(ByteArray::<N>::deserialize(deserializer)?.0))
        }
    }

    #[serde_as]
    #[derive(Serialize, Deserialize)]
    /// A wrapper for serializing and deserializing arrays as fixed byte sequences that can fail.
    pub struct TryByteArray<const N: usize>(#[serde_as(as = "Bytes")] [u8; N]);
    impl<T: TryToArray<u8, N>, const N: usize> SerializeAs<T> for TryByteArray<N> {
        fn serialize_as<S>(value: &T, serializer: S) -> Result<S::Ok, S::Error>
        where
            S: serde::Serializer,
        {
            ByteArray(value.try_to_array().map_err(serde::ser::Error::custom)?)
                .serialize(serializer)
        }
    }
    impl<'de, T: TryFromArray<u8, N>, const N: usize> DeserializeAs<'de, T> for TryByteArray<N> {
        fn deserialize_as<D>(deserializer: D) -> Result<T, D::Error>
        where
            D: serde::Deserializer<'de>,
        {
            Ok(
                T::try_from_array(ByteArray::<N>::deserialize(deserializer)?.0)
                    .map_err(serde::de::Error::custom)?,
            )
        }
    }
}

mod bytes {
    use serde::{Deserialize, Serialize};
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
    pub struct BytesVec<T: ToFromBytes>(T);

    impl<T: ToFromBytes> SerializeAs<T> for BytesVec<T> {
        fn serialize_as<S>(value: &T, serializer: S) -> Result<S::Ok, S::Error>
        where
            S: serde::Serializer,
        {
            value.to_bytes().serialize(serializer)
        }
    }

    impl<'de, T: ToFromBytes> DeserializeAs<'de, T> for BytesVec<T> {
        fn deserialize_as<D>(deserializer: D) -> Result<T, D::Error>
        where
            D: serde::Deserializer<'de>,
        {
            T::from_bytes(<Vec<u8>>::deserialize(deserializer)?.as_slice())
                .map_err(serde::de::Error::custom)
        }
    }
}
