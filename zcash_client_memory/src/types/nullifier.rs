use std::collections::BTreeMap;

use serde::{Deserialize, Serialize};
use serde_with::serde_as;
use zcash_primitives::consensus::BlockHeight;
use zcash_protocol::PoolType;

/// Maps a block height and transaction (i.e. transaction locator) index to a nullifier.
pub(crate) struct NullifierMap(BTreeMap<Nullifier, (BlockHeight, u32)>);

impl NullifierMap {
    pub fn new() -> Self {
        Self(BTreeMap::new())
    }
    pub fn insert(&mut self, height: BlockHeight, index: u32, nullifier: Nullifier) {
        self.0.insert(nullifier, (height, index));
    }

    pub fn get(&self, nullifier: &Nullifier) -> Option<&(BlockHeight, u32)> {
        self.0.get(nullifier)
    }
}

#[serde_as]
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
pub(crate) enum Nullifier {
    #[cfg(feature = "orchard")]
    Orchard(#[serde_as(as = "serialization::OrchardNullifierWrapper")] orchard::note::Nullifier),
    Sapling(#[serde_as(as = "serialization::SaplingNullifierWrapper")] sapling::Nullifier),
}

impl Nullifier {
    pub(crate) fn _pool(&self) -> PoolType {
        match self {
            #[cfg(feature = "orchard")]
            Nullifier::Orchard(_) => PoolType::ORCHARD,
            Nullifier::Sapling(_) => PoolType::SAPLING,
        }
    }
}
#[cfg(feature = "orchard")]
impl From<orchard::note::Nullifier> for Nullifier {
    fn from(n: orchard::note::Nullifier) -> Self {
        Nullifier::Orchard(n)
    }
}
impl From<sapling::Nullifier> for Nullifier {
    fn from(n: sapling::Nullifier) -> Self {
        Nullifier::Sapling(n)
    }
}

mod serialization {
    use serde::Deserialize as _;
    use serde::Serialize as _;

    #[cfg(feature = "orchard")]
    pub(crate) struct OrchardNullifierWrapper;
    #[cfg(feature = "orchard")]
    impl serde_with::SerializeAs<orchard::note::Nullifier> for OrchardNullifierWrapper {
        fn serialize_as<S>(
            value: &orchard::note::Nullifier,
            serializer: S,
        ) -> Result<S::Ok, S::Error>
        where
            S: serde::Serializer,
        {
            value.to_bytes().serialize(serializer)
        }
    }
    #[cfg(feature = "orchard")]
    impl<'de> serde_with::DeserializeAs<'de, orchard::note::Nullifier> for OrchardNullifierWrapper {
        fn deserialize_as<D>(deserializer: D) -> Result<orchard::note::Nullifier, D::Error>
        where
            D: serde::Deserializer<'de>,
        {
            Ok(
                orchard::note::Nullifier::from_bytes(&<[u8; 32]>::deserialize(deserializer)?)
                    .into_option()
                    .ok_or_else(|| serde::de::Error::custom("Invalid nullifier"))?,
            )
        }
    }

    pub(crate) struct SaplingNullifierWrapper;
    impl serde_with::SerializeAs<sapling::Nullifier> for SaplingNullifierWrapper {
        fn serialize_as<S>(value: &sapling::Nullifier, serializer: S) -> Result<S::Ok, S::Error>
        where
            S: serde::Serializer,
        {
            value.0.serialize(serializer)
        }
    }
    impl<'de> serde_with::DeserializeAs<'de, sapling::Nullifier> for SaplingNullifierWrapper {
        fn deserialize_as<D>(deserializer: D) -> Result<sapling::Nullifier, D::Error>
        where
            D: serde::Deserializer<'de>,
        {
            Ok(sapling::Nullifier(<[u8; 32]>::deserialize(deserializer)?))
        }
    }
}
