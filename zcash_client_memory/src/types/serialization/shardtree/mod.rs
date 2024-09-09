mod frontier;
mod tree;
pub use frontier::*;
use group::ff::BitViewSized;
pub use tree::*;

use std::io;

use crate::ToFromBytes;

use super::{ToArray, TryFromArray};

impl ToFromBytes for sapling::Node {
    fn to_bytes(&self) -> Vec<u8> {
        self.to_bytes().to_vec()
    }

    fn from_bytes(bytes: &[u8]) -> io::Result<Self> {
        let repr: [u8; 32] = bytes.try_into().map_err(|_| {
            io::Error::new(
                io::ErrorKind::InvalidData,
                "Invalid length for Jubjub base field value.",
            )
        })?;
        Option::from(Self::from_bytes(repr)).ok_or_else(|| {
            io::Error::new(
                io::ErrorKind::InvalidData,
                "Non-canonical encoding of Jubjub base field value.",
            )
        })
    }
}

impl ToArray<u8, 32> for sapling::Node {
    fn to_arr(&self) -> [u8; 32] {
        self.to_bytes()
    }
}
impl TryFromArray<u8, 32> for sapling::Node {
    type Error = io::Error;
    fn from_arr(arr: [u8; 32]) -> Result<Self, Self::Error> {
        Self::from_bytes(arr).into_option().ok_or_else(|| {
            io::Error::new(
                io::ErrorKind::InvalidData,
                "Non-canonical encoding of Jubjub base field value.",
            )
        })
    }
}
#[cfg(feature = "orchard")]
impl ToArray<u8, 32> for orchard::tree::MerkleHashOrchard {
    fn to_arr(&self) -> [u8; 32] {
        self.to_bytes()
    }
}
#[cfg(feature = "orchard")]
impl TryFromArray<u8, 32> for orchard::tree::MerkleHashOrchard {
    type Error = io::Error;
    fn from_arr(arr: [u8; 32]) -> Result<Self, Self::Error> {
        Self::from_bytes(&arr).into_option().ok_or_else(|| {
            io::Error::new(
                io::ErrorKind::InvalidData,
                "Non-canonical encoding of Pallas base field value.",
            )
        })
    }
}
#[cfg(feature = "orchard")]
impl ToFromBytes for orchard::tree::MerkleHashOrchard {
    fn to_bytes(&self) -> Vec<u8> {
        self.to_bytes().to_vec()
    }

    fn from_bytes(bytes: &[u8]) -> io::Result<Self> {
        let repr: [u8; 32] = bytes.try_into().map_err(|_| {
            io::Error::new(
                io::ErrorKind::InvalidData,
                "Invalid length for Pallas base field value.",
            )
        })?;
        <Option<_>>::from(Self::from_bytes(&repr)).ok_or_else(|| {
            io::Error::new(
                io::ErrorKind::InvalidData,
                "Non-canonical encoding of Pallas base field value.",
            )
        })
    }
}
// #[cfg(feature = "orchard")]
// pub use orchard::*;
// #[cfg(feature = "orchard")]
// mod orchard {
//     use crate::{ToArray, ToFromBytes, TryFromArray};
//     use std::io;
//     impl ToArray<u8, 32> for orchard::tree::MerkleHashOrchard {
//         fn to_arr(&self) -> [u8; 32] {
//             self.to_bytes()
//         }
//     }

//     impl TryFromArray<u8, 32> for orchard::tree::MerkleHashOrchard {
//         type Error = io::Error;
//         fn from_arr(arr: [u8; 32]) -> Result<Self, Self::Error> {
//             Self::from_bytes(&arr).into_option().ok_or_else(|| {
//                 io::Error::new(
//                     io::ErrorKind::InvalidData,
//                     "Non-canonical encoding of Pallas base field value.",
//                 )
//             })
//         }
//     }

//     impl ToFromBytes for orchard::tree::MerkleHashOrchard {
//         fn to_bytes(&self) -> Vec<u8> {
//             self.to_bytes().to_vec()
//         }

//         fn from_bytes(bytes: &[u8]) -> io::Result<Self> {
//             let repr: [u8; 32] = bytes.try_into().map_err(|_| {
//                 io::Error::new(
//                     io::ErrorKind::InvalidData,
//                     "Invalid length for Pallas base field value.",
//                 )
//             })?;
//             <Option<_>>::from(Self::from_bytes(&repr)).ok_or_else(|| {
//                 io::Error::new(
//                     io::ErrorKind::InvalidData,
//                     "Non-canonical encoding of Pallas base field value.",
//                 )
//             })
//         }
//     }
// }
