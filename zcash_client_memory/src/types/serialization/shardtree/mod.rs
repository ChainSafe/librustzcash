mod frontier;
mod tree;
pub use frontier::*;
use incrementalmerkletree::Hashable;
pub use tree::*;

use std::io;

use super::{ToArray, TryFromArray};

pub trait TreeNode<const N: usize>:
    Clone + Hashable + PartialEq + TryFromArray<u8, N> + ToArray<u8, N>
{
}

impl TreeNode<32> for sapling::Node {}

impl ToArray<u8, 32> for sapling::Node {
    fn to_array(&self) -> [u8; 32] {
        self.to_bytes()
    }
}
impl TryFromArray<u8, 32> for sapling::Node {
    type Error = io::Error;
    fn try_from_array(arr: [u8; 32]) -> Result<Self, Self::Error> {
        Self::from_bytes(arr).into_option().ok_or_else(|| {
            io::Error::new(
                io::ErrorKind::InvalidData,
                "Non-canonical encoding of Jubjub base field value.",
            )
        })
    }
}

#[cfg(feature = "orchard")]
mod _orchard {
    use super::*;
    impl TreeNode<32> for orchard::tree::MerkleHashOrchard {}
    impl ToArray<u8, 32> for orchard::tree::MerkleHashOrchard {
        fn to_array(&self) -> [u8; 32] {
            self.to_bytes()
        }
    }
    impl TryFromArray<u8, 32> for orchard::tree::MerkleHashOrchard {
        type Error = io::Error;
        fn try_from_array(arr: [u8; 32]) -> Result<Self, Self::Error> {
            Self::from_bytes(&arr).into_option().ok_or_else(|| {
                io::Error::new(
                    io::ErrorKind::InvalidData,
                    "Non-canonical encoding of Pallas base field value.",
                )
            })
        }
    }
}
