use serde::{Deserialize, Deserializer, Serialize, Serializer};
use zcash_protocol::consensus::MAIN_NETWORK;

#[derive(Serialize, Deserialize)]
pub struct DiversifierIndexDef([u8; 11]);

impl From<DiversifierIndexDef> for zip32::DiversifierIndex {
    fn from(wrapper: DiversifierIndexDef) -> Self {
        zip32::DiversifierIndex::from(wrapper.0)
    }
}

impl From<zip32::DiversifierIndex> for DiversifierIndexDef {
    fn from(diversifier_index: zip32::DiversifierIndex) -> Self {
        DiversifierIndexDef(*diversifier_index.as_bytes())
    }
}

pub struct UnifiedAddressDef(zcash_keys::address::UnifiedAddress);

impl From<UnifiedAddressDef> for zcash_keys::address::UnifiedAddress {
    fn from(wrapper: UnifiedAddressDef) -> Self {
        wrapper.0
    }
}

impl From<zcash_keys::address::UnifiedAddress> for UnifiedAddressDef {
    fn from(unified_address: zcash_keys::address::UnifiedAddress) -> Self {
        UnifiedAddressDef(unified_address)
    }
}

// use the canonical string encoding assuming mainnet for serializing unified addresses

impl Serialize for UnifiedAddressDef {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: Serializer,
    {
        self.0.encode(&MAIN_NETWORK).serialize(serializer)
    }
}

impl<'a> Deserialize<'a> for UnifiedAddressDef {
    fn deserialize<D>(deserializer: D) -> Result<UnifiedAddressDef, D::Error>
    where
        D: Deserializer<'a>,
    {
        let b = <String>::deserialize(deserializer)?;
        if let Some(zcash_keys::address::Address::Unified(unified_address)) =
            zcash_keys::address::Address::decode(&MAIN_NETWORK, &b)
        {
            Ok(UnifiedAddressDef(unified_address))
        } else {
            Err(serde::de::Error::custom("Invalid unified address"))
        }
    }
}
