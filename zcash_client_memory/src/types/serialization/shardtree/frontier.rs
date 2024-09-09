use std::fmt::Display;

use incrementalmerkletree::frontier::{self, Frontier, NonEmptyFrontier};
use incrementalmerkletree::Position;

use serde::{Deserialize, Deserializer, Serialize};
use serde_with::SerializeAs;
use serde_with::{de::DeserializeAs, serde_as};

use crate::{ToArray, TryFromArray};

pub struct FrontierDef;

impl<H: ToArray<u8, 32> + Clone, const DEPTH: u8> SerializeAs<Frontier<H, DEPTH>> for FrontierDef {
    fn serialize_as<S>(value: &Frontier<H, DEPTH>, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: serde::Serializer,
    {
        #[serde_as]
        #[derive(Serialize)]
        struct FrontierSer<'a, H: ToArray<u8, 32>> {
            #[serde_as(as = "Option<&'a NonEmptyFrontierDef>")]
            frontier: &'a Option<&'a NonEmptyFrontier<H>>,
        }

        FrontierSer {
            frontier: &value.value(),
        }
        .serialize(serializer)
    }
}
impl<'de, H: TryFromArray<u8, 32, Error = E>, E: Display, const DEPTH: u8>
    DeserializeAs<'de, Frontier<H, DEPTH>> for FrontierDef
{
    fn deserialize_as<D>(deserializer: D) -> Result<Frontier<H, DEPTH>, D::Error>
    where
        D: Deserializer<'de>,
    {
        #[derive(Deserialize)]
        struct FrontierDe {
            frontier: Option<NonEmptyFrontierDe>,
        }
        let frontier = FrontierDe::deserialize(deserializer)?;
        frontier
            .frontier
            .map(|frontier_de| {
                let position = Position::from(frontier_de.position);
                let leaf = H::try_from_array(frontier_de.leaf).map_err(serde::de::Error::custom)?;
                let ommers = frontier_de
                    .ommers
                    .into_iter()
                    .map(|o| H::try_from_array(o).map_err(serde::de::Error::custom))
                    .collect::<Result<Vec<_>, _>>()?;
                frontier::Frontier::from_parts(position, leaf, ommers).map_err(|_e| {
                    serde::de::Error::custom("failed to construct frontier from parts")
                })
            })
            .transpose()?
            .ok_or_else(|| serde::de::Error::missing_field("frontier"))
    }
}

struct NonEmptyFrontierDef;

impl<T> SerializeAs<NonEmptyFrontier<T>> for NonEmptyFrontierDef
where
    T: ToArray<u8, 32>,
{
    fn serialize_as<S>(value: &NonEmptyFrontier<T>, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: serde::Serializer,
    {
        #[derive(Serialize)]
        struct NonEmptyFrontierSer<'a> {
            pub position: u64,
            pub leaf: &'a [u8; 32],
            pub ommers: &'a [[u8; 32]],
        }

        let ommer = value
            .ommers()
            .iter()
            .map(|o| o.to_array())
            .collect::<Vec<_>>();

        let x = NonEmptyFrontierSer {
            position: value.position().into(),
            leaf: &value.leaf().to_array(),
            ommers: ommer.as_slice(),
        };

        x.serialize(serializer)
    }
}
#[derive(Deserialize)]
struct NonEmptyFrontierDe {
    pub position: u64,
    pub leaf: [u8; 32],
    pub ommers: Vec<[u8; 32]>,
}

impl<'de, T: TryFromArray<u8, 32, Error = E>, E: Display> DeserializeAs<'de, NonEmptyFrontier<T>>
    for NonEmptyFrontierDef
{
    fn deserialize_as<D>(deserializer: D) -> Result<NonEmptyFrontier<T>, D::Error>
    where
        D: Deserializer<'de>,
    {
        let frontier = NonEmptyFrontierDe::deserialize(deserializer)?;
        NonEmptyFrontier::from_parts(
            frontier.position.into(),
            T::try_from_array(frontier.leaf).map_err(serde::de::Error::custom)?,
            frontier
                .ommers
                .into_iter()
                .map(|o| T::try_from_array(o).map_err(serde::de::Error::custom))
                .collect::<Result<Vec<_>, _>>()?,
        )
        .map_err(|_| serde::de::Error::custom("Failed to construct frontier from parts"))
    }
}
