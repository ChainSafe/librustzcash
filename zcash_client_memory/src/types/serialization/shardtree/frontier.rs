use incrementalmerkletree::frontier::{self, Frontier, NonEmptyFrontier};
use incrementalmerkletree::Position;
use serde::ser::SerializeStruct;

use serde_with::{de::DeserializeAs, de::DeserializeAsWrap, ser::SerializeAsWrap};
use serde_with::{FromInto, SerializeAs};

use crate::{ToFromBytes, ToFromBytesWrapper};

pub struct FrontierWrapper;
impl<T: ToFromBytes + Clone, const DEPTH: u8> SerializeAs<Frontier<T, DEPTH>> for FrontierWrapper {
    fn serialize_as<S>(value: &Frontier<T, DEPTH>, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: serde::Serializer,
    {
        let mut s = serializer.serialize_struct("Frontier", 1)?;
        s.serialize_field(
            "frontier",
            &SerializeAsWrap::<_, Option<NonEmptyFrontierWrapper>>::new(&value.value().cloned()),
        )?;
        s.end()
    }
}
impl<'de, T: ToFromBytes + Clone, const DEPTH: u8> DeserializeAs<'de, Frontier<T, DEPTH>>
    for FrontierWrapper
{
    fn deserialize_as<D>(deserializer: D) -> Result<Frontier<T, DEPTH>, D::Error>
    where
        D: serde::Deserializer<'de>,
    {
        struct Visitor<T, const DEPTH: u8>(std::marker::PhantomData<T>);
        impl<T, const DEPTH: u8> Visitor<T, DEPTH> {
            fn new() -> Self {
                Self(std::marker::PhantomData)
            }
        }
        impl<'de, T: ToFromBytes + Clone, const DEPTH: u8> serde::de::Visitor<'de> for Visitor<T, DEPTH> {
            type Value = Frontier<T, DEPTH>;
            fn expecting(&self, formatter: &mut std::fmt::Formatter) -> std::fmt::Result {
                formatter.write_str("struct Frontier")
            }
            fn visit_map<A>(self, mut map: A) -> Result<Frontier<T, DEPTH>, A::Error>
            where
                A: serde::de::MapAccess<'de>,
            {
                let mut frontier = None;
                while let Some(key) = map.next_key()? {
                    match key {
                        "frontier" => {
                            frontier = map
                                .next_value::<Option<
                                    DeserializeAsWrap<NonEmptyFrontier<T>, NonEmptyFrontierWrapper>,
                                >>()?
                                .map(|f| f.into_inner());
                        }
                        _ => {
                            return Err(serde::de::Error::unknown_field(key, &["frontier"]));
                        }
                    }
                }
                frontier
                    .map(NonEmptyFrontier::into_parts)
                    .map(|(p, l, o)| {
                        frontier::Frontier::from_parts(p, l, o).map_err(|_e| {
                            serde::de::Error::custom("failed to construct frontier from parts")
                        })
                    })
                    .transpose()?
                    .ok_or_else(|| serde::de::Error::missing_field("frontier"))
            }
        }
        deserializer.deserialize_struct("Frontier", &["frontier"], Visitor::<T, DEPTH>::new())
    }
}

pub struct NonEmptyFrontierWrapper;

impl<T: ToFromBytes> SerializeAs<NonEmptyFrontier<T>> for NonEmptyFrontierWrapper {
    fn serialize_as<S>(value: &NonEmptyFrontier<T>, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: serde::Serializer,
    {
        let ommers = value
            .ommers()
            .iter()
            .map(|o| SerializeAsWrap::<_, ToFromBytesWrapper<T>>::new(o))
            .collect::<Vec<_>>();
        let mut s = serializer.serialize_struct("NonEmptyFrontier", 3)?;
        s.serialize_field(
            "position",
            &SerializeAsWrap::<_, FromInto<u64>>::new(&value.position()),
        )?;
        s.serialize_field(
            "leaf",
            &SerializeAsWrap::<_, ToFromBytesWrapper<T>>::new(&value.leaf()),
        )?;
        s.serialize_field("ommers", &ommers)?;
        s.end()
    }
}

impl<'de, T: ToFromBytes> DeserializeAs<'de, NonEmptyFrontier<T>> for NonEmptyFrontierWrapper {
    fn deserialize_as<D>(deserializer: D) -> Result<NonEmptyFrontier<T>, D::Error>
    where
        D: serde::Deserializer<'de>,
    {
        struct Visitor<T>(std::marker::PhantomData<T>);
        impl<T> Visitor<T> {
            fn new() -> Self {
                Self(std::marker::PhantomData)
            }
        }
        impl<'de, T: ToFromBytes> serde::de::Visitor<'de> for Visitor<T> {
            type Value = NonEmptyFrontier<T>;
            fn expecting(&self, formatter: &mut std::fmt::Formatter) -> std::fmt::Result {
                formatter.write_str("struct OrchardNote")
            }
            fn visit_map<A>(self, mut map: A) -> Result<Self::Value, A::Error>
            where
                A: serde::de::MapAccess<'de>,
            {
                let mut position = None;
                let mut leaf = None;
                let mut ommers = None;
                while let Some(key) = map.next_key()? {
                    match key {
                        "position" => {
                            position = Some(
                                map.next_value::<DeserializeAsWrap<Position, FromInto<u64>>>()?,
                            );
                        }
                        "leaf" => {
                            leaf = Some(
                                map.next_value::<DeserializeAsWrap<T, ToFromBytesWrapper<T>>>()?,
                            );
                        }
                        "ommers" => {
                            ommers = Some(
                                map.next_value::<Vec<DeserializeAsWrap<T, ToFromBytesWrapper<T>>>>(
                                )?,
                            );
                        }
                        _ => {
                            return Err(serde::de::Error::unknown_field(
                                key,
                                &["recipient", "value", "rho", "rseed"],
                            ));
                        }
                    }
                }
                let position = position
                    .ok_or_else(|| serde::de::Error::missing_field("position"))?
                    .into_inner();
                let leaf = leaf
                    .ok_or_else(|| serde::de::Error::missing_field("leaf"))?
                    .into_inner();
                let ommers = ommers
                    .ok_or_else(|| serde::de::Error::missing_field("ommers"))?
                    .into_iter()
                    .map(|o| o.into_inner())
                    .collect();

                NonEmptyFrontier::from_parts(position, leaf, ommers).map_err(|_e| {
                    serde::de::Error::custom("Failed to deserialize non-empty frontier")
                })
            }
        }
        deserializer.deserialize_struct(
            "NonEmptyFrontier",
            &["position", "leaf", "ommers"],
            Visitor::<T>::new(),
        )
    }
}
