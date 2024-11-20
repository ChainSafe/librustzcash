use std::collections::BTreeMap;

use zcash_primitives::consensus::BlockHeight;
use zcash_protocol::PoolType;

/// Maps a nullifier to the block height and transaction index where it was spent.
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

#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord)]
pub(crate) enum Nullifier {
    Sapling(sapling::Nullifier),
    #[cfg(feature = "orchard")]
    Orchard(orchard::note::Nullifier),
}

enum NullifierSerDe {
    Sapling([u8; 32]),
    Orchard([u8; 32]),
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
