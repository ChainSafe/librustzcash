use std::collections::BTreeMap;

use crate::types::serialization::*;
use serde::{Deserialize, Serialize};
use serde_with::serde_as;
use serde_with::FromInto;
use zcash_primitives::consensus::BlockHeight;
use zcash_protocol::PoolType;

/// Maps a block height and transaction (i.e. transaction locator) index to a nullifier.
#[serde_as]
#[derive(Serialize, Deserialize)]
pub(crate) struct NullifierMap(
    #[serde_as(as = "BTreeMap<_, (FromInto<u32>, _)>")] BTreeMap<Nullifier, (BlockHeight, u32)>,
);

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
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord)]
pub(crate) enum Nullifier {
    Sapling(sapling::Nullifier),
    #[cfg(feature = "orchard")]
    Orchard(orchard::note::Nullifier),
}
#[derive(Serialize, Deserialize)]
enum NullifierSerDe {
    Sapling([u8; 32]),
    Orchard([u8; 32]),
}
impl Serialize for Nullifier {
    fn serialize<S: serde::Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        match self {
            Nullifier::Sapling(n) => NullifierSerDe::Sapling(n.to_array()).serialize(serializer),
            #[cfg(feature = "orchard")]
            Nullifier::Orchard(n) => NullifierSerDe::Orchard(n.to_array()).serialize(serializer),
        }
    }
}

impl<'de> Deserialize<'de> for Nullifier {
    fn deserialize<D: serde::Deserializer<'de>>(deserializer: D) -> Result<Self, D::Error> {
        let n = NullifierSerDe::deserialize(deserializer)?;
        Ok(match n {
            NullifierSerDe::Sapling(n) => Nullifier::Sapling(
                sapling::Nullifier::try_from_array(n).map_err(serde::de::Error::custom)?,
            ),
            #[cfg(feature = "orchard")]
            NullifierSerDe::Orchard(n) => Nullifier::Orchard(
                orchard::note::Nullifier::try_from_array(n).map_err(serde::de::Error::custom)?,
            ),
            #[cfg(not(feature = "orchard"))]
            _ => return Err(serde::de::Error::custom("Invalid nullifier")),
        })
    }
}

impl Nullifier {
    pub(crate) fn _pool(&self) -> PoolType {
        match self {
            Nullifier::Sapling(_) => PoolType::SAPLING,
            #[cfg(feature = "orchard")]
            Nullifier::Orchard(_) => PoolType::ORCHARD,
        }
    }
}

impl From<sapling::Nullifier> for Nullifier {
    fn from(n: sapling::Nullifier) -> Self {
        Nullifier::Sapling(n)
    }
}

#[cfg(feature = "orchard")]
impl From<orchard::note::Nullifier> for Nullifier {
    fn from(n: orchard::note::Nullifier) -> Self {
        Nullifier::Orchard(n)
    }
}
