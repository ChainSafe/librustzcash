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
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
pub(crate) enum Nullifier {
    #[cfg(feature = "orchard")]
    Orchard(#[serde_as(as = "OrchardNullifierWrapper")] orchard::note::Nullifier),
    Sapling(#[serde_as(as = "SaplingNullifierWrapper")] sapling::Nullifier),
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
