use std::convert::Infallible;

use super::{FromArray, ToArray, TryFromArray};

impl FromArray<u8, 32> for sapling::Nullifier {
    fn from_array(arr: [u8; 32]) -> Self {
        sapling::Nullifier(arr)
    }
}

impl ToArray<u8, 32> for sapling::Nullifier {
    fn to_array(&self) -> [u8; 32] {
        self.0
    }
}

#[cfg(feature = "orchard")]
mod _orchard {
    use super::*;
    use std::io;
    impl TryFromArray<u8, 32> for orchard::note::Nullifier {
        type Error = io::Error;

        fn try_from_array(arr: [u8; 32]) -> Result<Self, Self::Error> {
            orchard::note::Nullifier::from_bytes(&arr)
                .into_option()
                .ok_or_else(|| {
                    io::Error::new(io::ErrorKind::InvalidData, "Invalid sapling nullifier")
                })
        }
    }

    impl ToArray<u8, 32> for orchard::note::Nullifier {
        fn to_array(&self) -> [u8; 32] {
            (*self).to_bytes()
        }
    }
}
