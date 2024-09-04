use serde::{ser::SerializeStruct, Deserialize, Serialize};
use serde_with::FromInto;
use serde_with::TryFromInto;
use serde_with::{ser::SerializeAsWrap, serde_as};
use zcash_client_backend::{
    data_api::{AccountPurpose, AccountSource},
    wallet::NoteId,
};
use zcash_primitives::{block::BlockHash, transaction::TxId};
use zcash_protocol::consensus::BlockHeight;
use zcash_protocol::memo::Memo;
use zcash_protocol::{memo::MemoBytes, ShieldedProtocol};
use zip32::fingerprint::SeedFingerprint;

#[serde_as]
#[derive(Serialize, Deserialize)]
#[serde(remote = "AccountSource")]
pub enum AccountSourceWrapper {
    /// An account derived from a known seed.
    Derived {
        #[serde_as(as = "SeedFingerprintWrapper")]
        seed_fingerprint: SeedFingerprint,
        #[serde_as(as = "TryFromInto<u32>")]
        account_index: zip32::AccountId,
    },

    /// An account imported from a viewing key.
    Imported {
        #[serde_as(as = "AccountPurposeWrapper")]
        purpose: AccountPurpose,
    },
}
// Provide a conversion to construct the remote type.
impl From<AccountSourceWrapper> for AccountSource {
    fn from(def: AccountSourceWrapper) -> AccountSource {
        match def {
            AccountSourceWrapper::Derived {
                seed_fingerprint,
                account_index,
            } => AccountSource::Derived {
                seed_fingerprint,
                account_index,
            },
            AccountSourceWrapper::Imported { purpose } => AccountSource::Imported { purpose },
        }
    }
}

impl serde_with::SerializeAs<AccountSource> for AccountSourceWrapper {
    fn serialize_as<S>(value: &AccountSource, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: serde::Serializer,
    {
        AccountSourceWrapper::serialize(value, serializer)
    }
}

impl<'de> serde_with::DeserializeAs<'de, AccountSource> for AccountSourceWrapper {
    fn deserialize_as<D>(deserializer: D) -> Result<AccountSource, D::Error>
    where
        D: serde::Deserializer<'de>,
    {
        AccountSourceWrapper::deserialize(deserializer).map(Into::into)
    }
}

pub(crate) struct TxIdWrapper;

impl serde_with::SerializeAs<TxId> for TxIdWrapper {
    fn serialize_as<S>(value: &TxId, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: serde::Serializer,
    {
        value.as_ref().serialize(serializer)
    }
}

impl<'de> serde_with::DeserializeAs<'de, TxId> for TxIdWrapper {
    fn deserialize_as<D>(deserializer: D) -> Result<TxId, D::Error>
    where
        D: serde::Deserializer<'de>,
    {
        Ok(TxId::from_bytes(<[u8; 32]>::deserialize(deserializer)?))
    }
}

pub(crate) struct ShieldedProtocolWrapper;
impl serde_with::SerializeAs<ShieldedProtocol> for ShieldedProtocolWrapper {
    fn serialize_as<S>(value: &ShieldedProtocol, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: serde::Serializer,
    {
        match value {
            ShieldedProtocol::Sapling => serializer.serialize_str("Sapling"),
            ShieldedProtocol::Orchard => serializer.serialize_str("Orchard"),
        }
    }
}

impl<'de> serde_with::DeserializeAs<'de, ShieldedProtocol> for ShieldedProtocolWrapper {
    fn deserialize_as<D>(deserializer: D) -> Result<ShieldedProtocol, D::Error>
    where
        D: serde::Deserializer<'de>,
    {
        let s = String::deserialize(deserializer)?;
        match s.as_str() {
            "Sapling" => Ok(ShieldedProtocol::Sapling),
            "Orchard" => Ok(ShieldedProtocol::Orchard),
            _ => Err(serde::de::Error::custom("Invalid shielded protocol")),
        }
    }
}

pub struct MemoBytesWrapper;
impl serde_with::SerializeAs<MemoBytes> for MemoBytesWrapper {
    fn serialize_as<S>(value: &MemoBytes, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: serde::Serializer,
    {
        value.as_slice().serialize(serializer)
    }
}

impl<'de> serde_with::DeserializeAs<'de, MemoBytes> for MemoBytesWrapper {
    fn deserialize_as<D>(deserializer: D) -> Result<MemoBytes, D::Error>
    where
        D: serde::Deserializer<'de>,
    {
        let b = <Vec<u8>>::deserialize(deserializer)?;
        Ok(
            MemoBytes::from_bytes(&b)
                .map_err(|_| serde::de::Error::custom("Invalid memo bytes"))?,
        )
    }
}

impl serde_with::SerializeAs<Memo> for MemoBytesWrapper {
    fn serialize_as<S>(value: &Memo, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: serde::Serializer,
    {
        value.encode().as_slice().serialize(serializer)
    }
}

impl<'de> serde_with::DeserializeAs<'de, Memo> for MemoBytesWrapper {
    fn deserialize_as<D>(deserializer: D) -> Result<Memo, D::Error>
    where
        D: serde::Deserializer<'de>,
    {
        let b = <Vec<u8>>::deserialize(deserializer)?;
        Ok(Memo::from_bytes(&b).map_err(|_| serde::de::Error::custom("Invalid memo"))?)
    }
}

#[serde_as]
#[derive(Serialize, Deserialize)]
#[serde(remote = "NoteId")]
pub(crate) struct NoteIdWrapper {
    #[serde(getter = "NoteId::txid")]
    #[serde_as(as = "TxIdWrapper")]
    txid: TxId,
    #[serde(getter = "NoteId::protocol")]
    #[serde_as(as = "ShieldedProtocolWrapper")]
    protocol: ShieldedProtocol,
    #[serde(getter = "NoteId::output_index")]
    output_index: u16,
}

impl From<NoteIdWrapper> for NoteId {
    fn from(def: NoteIdWrapper) -> NoteId {
        NoteId::new(def.txid, def.protocol, def.output_index)
    }
}

impl serde_with::SerializeAs<NoteId> for NoteIdWrapper {
    fn serialize_as<S>(value: &NoteId, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: serde::Serializer,
    {
        NoteIdWrapper::serialize(value, serializer)
    }
}

impl<'de> serde_with::DeserializeAs<'de, NoteId> for NoteIdWrapper {
    fn deserialize_as<D>(deserializer: D) -> Result<NoteId, D::Error>
    where
        D: serde::Deserializer<'de>,
    {
        NoteIdWrapper::deserialize(deserializer).map(Into::into)
    }
}

pub(crate) struct BlockHashWrapper;
impl serde_with::SerializeAs<BlockHash> for BlockHashWrapper {
    fn serialize_as<S>(value: &BlockHash, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: serde::Serializer,
    {
        value.0.serialize(serializer)
    }
}
impl<'de> serde_with::DeserializeAs<'de, BlockHash> for BlockHashWrapper {
    fn deserialize_as<D>(deserializer: D) -> Result<BlockHash, D::Error>
    where
        D: serde::Deserializer<'de>,
    {
        Ok(BlockHash(<[u8; 32]>::deserialize(deserializer)?))
    }
}

pub(crate) struct AccountPurposeWrapper;
impl serde_with::SerializeAs<AccountPurpose> for AccountPurposeWrapper {
    fn serialize_as<S>(value: &AccountPurpose, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: serde::Serializer,
    {
        match value {
            AccountPurpose::Spending => serializer.serialize_str("Spending"),
            AccountPurpose::ViewOnly => serializer.serialize_str("ViewOnly"),
        }
    }
}
impl<'de> serde_with::DeserializeAs<'de, AccountPurpose> for AccountPurposeWrapper {
    fn deserialize_as<D>(deserializer: D) -> Result<AccountPurpose, D::Error>
    where
        D: serde::Deserializer<'de>,
    {
        let s = String::deserialize(deserializer)?;
        match s.as_str() {
            "Spending" => Ok(AccountPurpose::Spending),
            "ViewOnly" => Ok(AccountPurpose::ViewOnly),
            _ => Err(serde::de::Error::custom("Invalid account purpose")),
        }
    }
}

pub(crate) struct SeedFingerprintWrapper;
impl serde_with::SerializeAs<SeedFingerprint> for SeedFingerprintWrapper {
    fn serialize_as<S>(value: &SeedFingerprint, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: serde::Serializer,
    {
        value.to_bytes().serialize(serializer)
    }
}
impl<'de> serde_with::DeserializeAs<'de, SeedFingerprint> for SeedFingerprintWrapper {
    fn deserialize_as<D>(deserializer: D) -> Result<SeedFingerprint, D::Error>
    where
        D: serde::Deserializer<'de>,
    {
        Ok(SeedFingerprint::from_bytes(<[u8; 32]>::deserialize(
            deserializer,
        )?))
    }
}
#[serde_as]
#[derive(Serialize, Deserialize)]
#[serde(remote = "incrementalmerkletree::Address")]
pub struct TreeAddressWrapper {
    #[serde_as(as = "FromInto<u8>")]
    #[serde(getter = "incrementalmerkletree::Address::level")]
    level: incrementalmerkletree::Level,
    #[serde(getter = "incrementalmerkletree::Address::index")]
    index: u64,
}
impl From<TreeAddressWrapper> for incrementalmerkletree::Address {
    fn from(def: TreeAddressWrapper) -> incrementalmerkletree::Address {
        incrementalmerkletree::Address::from_parts(def.level, def.index)
    }
}
impl serde_with::SerializeAs<incrementalmerkletree::Address> for TreeAddressWrapper {
    fn serialize_as<S>(
        value: &incrementalmerkletree::Address,
        serializer: S,
    ) -> Result<S::Ok, S::Error>
    where
        S: serde::Serializer,
    {
        TreeAddressWrapper::serialize(value, serializer)
    }
}
impl<'de> serde_with::DeserializeAs<'de, incrementalmerkletree::Address> for TreeAddressWrapper {
    fn deserialize_as<D>(deserializer: D) -> Result<incrementalmerkletree::Address, D::Error>
    where
        D: serde::Deserializer<'de>,
    {
        TreeAddressWrapper::deserialize(deserializer).map(Into::into)
    }
}

pub mod arrays {
    use std::{convert::TryInto, marker::PhantomData};

    use serde::{
        de::{SeqAccess, Visitor},
        ser::SerializeTuple,
        Deserialize, Deserializer, Serialize, Serializer,
    };
    pub fn serialize<S: Serializer, T: Serialize, const N: usize>(
        data: &[T; N],
        ser: S,
    ) -> Result<S::Ok, S::Error> {
        let mut s = ser.serialize_tuple(N)?;
        for item in data {
            s.serialize_element(item)?;
        }
        s.end()
    }

    struct ArrayVisitor<T, const N: usize>(PhantomData<T>);

    impl<'de, T, const N: usize> Visitor<'de> for ArrayVisitor<T, N>
    where
        T: Deserialize<'de>,
    {
        type Value = [T; N];

        fn expecting(&self, formatter: &mut std::fmt::Formatter) -> std::fmt::Result {
            formatter.write_str(&format!("an array of length {}", N))
        }

        #[inline]
        fn visit_seq<A>(self, mut seq: A) -> Result<Self::Value, A::Error>
        where
            A: SeqAccess<'de>,
        {
            // can be optimized using MaybeUninit
            let mut data = Vec::with_capacity(N);
            for _ in 0..N {
                match (seq.next_element())? {
                    Some(val) => data.push(val),
                    None => return Err(serde::de::Error::invalid_length(N, &self)),
                }
            }
            match data.try_into() {
                Ok(arr) => Ok(arr),
                Err(_) => unreachable!(),
            }
        }
    }
    pub fn deserialize<'de, D, T, const N: usize>(deserializer: D) -> Result<[T; N], D::Error>
    where
        D: Deserializer<'de>,
        T: Deserialize<'de>,
    {
        deserializer.deserialize_tuple(N, ArrayVisitor::<T, N>(PhantomData))
    }
}
