use std::io;

use crate::ToFromBytes;

impl ToFromBytes for sapling::Nullifier {
    fn to_bytes(&self) -> Vec<u8> {
        self.0.to_vec()
    }

    fn from_bytes(bytes: &[u8]) -> io::Result<Self> {
        Ok(sapling::Nullifier(bytes.try_into().map_err(|e| {
            io::Error::new(io::ErrorKind::InvalidData, format!("{}", e))
        })?))
    }
}

#[cfg(feature = "orchard")]
impl ToFromBytes for orchard::note::Nullifier {
    fn to_bytes(&self) -> Vec<u8> {
        (*self).to_bytes().to_vec()
    }

    fn from_bytes(bytes: &[u8]) -> io::Result<Self> {
        orchard::note::Nullifier::from_bytes(
            bytes
                .try_into()
                .map_err(|e| io::Error::new(io::ErrorKind::InvalidData, format!("{}", e)))?,
        )
        .into_option()
        .ok_or_else(|| io::Error::new(io::ErrorKind::InvalidData, "Invalid sapling nullifier"))
    }
}
