mod received;
mod sent;

pub(crate) use received::{
    to_spendable_notes, ReceievdNoteSpends, ReceivedNote, ReceivedNoteTable,
};
pub(crate) use sent::SentNoteTable;

mod serialization {
    use crate::{
        proto::memwallet::{self as proto},
        Nullifier,
    };
    use jubjub::Fr;
    use zcash_client_backend::wallet::{Note, NoteId};
    use zcash_primitives::transaction::TxId;

    impl From<NoteId> for proto::NoteId {
        fn from(note_id: NoteId) -> Self {
            Self {
                tx_id: note_id.txid().as_ref().to_vec(),
                protocol: match note_id.protocol() {
                    zcash_protocol::ShieldedProtocol::Sapling => 0,
                    #[cfg(feature = "orchard")]
                    zcash_protocol::ShieldedProtocol::Orchard => 1,
                },
                output_index: note_id.output_index() as u32,
            }
        }
    }

    impl From<proto::NoteId> for NoteId {
        fn from(note_id: proto::NoteId) -> Self {
            Self::new(
                TxId::from_bytes(note_id.tx_id.clone().try_into().unwrap()),
                match note_id.protocol() {
                    proto::ShieldedProtocol::Sapling => zcash_protocol::ShieldedProtocol::Sapling,
                    #[cfg(feature = "orchard")]
                    proto::ShieldedProtocol::Orchard => zcash_protocol::ShieldedProtocol::Orchard,
                },
                note_id.output_index.try_into().unwrap(),
            )
        }
    }

    impl From<Nullifier> for proto::Nullifier {
        fn from(nullifier: Nullifier) -> Self {
            match nullifier {
                Nullifier::Sapling(n) => Self {
                    protocol: proto::ShieldedProtocol::Sapling.into(),
                    nullifier: n.to_vec(),
                },
                #[cfg(feature = "orchard")]
                Nullifier::Orchard(n) => Self {
                    protocol: proto::ShieldedProtocol::Orchard.into(),
                    nullifier: n.to_bytes().to_vec(),
                },
            }
        }
    }

    impl From<proto::Nullifier> for Nullifier {
        fn from(nullifier: proto::Nullifier) -> Self {
            match nullifier.protocol {
                0 => Nullifier::Sapling(
                    sapling::Nullifier::from_slice(&nullifier.nullifier).unwrap(),
                ),
                1 => Nullifier::Orchard(
                    orchard::note::Nullifier::from_bytes(&nullifier.nullifier.try_into().unwrap())
                        .unwrap(),
                ),
                _ => panic!("invalid protocol"),
            }
        }
    }

    impl From<Note> for proto::Note {
        fn from(note: Note) -> Self {
            match note {
                Note::Sapling(note) => Self {
                    protocol: proto::ShieldedProtocol::Sapling.into(),
                    recipient: note.recipient().to_bytes().to_vec(),
                    value: note.value().inner(),
                    rseed: match note.rseed() {
                        sapling::Rseed::AfterZip212(inner) => Some(proto::RSeed {
                            rseed_type: Some(proto::RSeedType::AfterZip212 as i32),
                            payload: inner.to_vec(),
                        }),
                        sapling::Rseed::BeforeZip212(inner) => Some(proto::RSeed {
                            rseed_type: Some(proto::RSeedType::BeforeZip212 as i32),
                            payload: inner.to_bytes().to_vec(),
                        }),
                    },
                    rho: None,
                },
                #[cfg(feature = "orchard")]
                Note::Orchard(note) => Self {
                    protocol: proto::ShieldedProtocol::Orchard.into(),
                    recipient: note.recipient().to_raw_address_bytes().to_vec(),
                    value: note.value().inner(),
                    rseed: Some(proto::RSeed {
                        rseed_type: None,
                        payload: note.rseed().as_bytes().to_vec(),
                    }),
                    rho: Some(note.rho().to_bytes().to_vec()),
                },
            }
        }
    }

    impl From<proto::Note> for Note {
        fn from(note: proto::Note) -> Self {
            match note.protocol {
                0 => {
                    let recipient =
                        sapling::PaymentAddress::from_bytes(&note.recipient.try_into().unwrap())
                            .unwrap();
                    let value = sapling::value::NoteValue::from_raw(note.value);
                    let rseed = match note.rseed {
                        Some(proto::RSeed {
                            rseed_type: Some(0),
                            payload,
                        }) => sapling::Rseed::BeforeZip212(
                            Fr::from_bytes(&payload.try_into().unwrap()).unwrap(),
                        ),
                        Some(proto::RSeed {
                            rseed_type: Some(1),
                            payload,
                        }) => sapling::Rseed::AfterZip212(payload.try_into().unwrap()),
                        _ => panic!("rseed is required"),
                    };
                    Self::Sapling(sapling::Note::from_parts(recipient, value, rseed))
                }
                1 => {
                    let recipient = orchard::Address::from_raw_address_bytes(
                        &note.recipient.try_into().unwrap(),
                    )
                    .unwrap();
                    let value = orchard::value::NoteValue::from_raw(note.value);
                    let rho =
                        orchard::note::Rho::from_bytes(&note.rho.unwrap().try_into().unwrap())
                            .unwrap();
                    let rseed = orchard::note::RandomSeed::from_bytes(
                        note.rseed.unwrap().payload.try_into().unwrap(),
                        &rho,
                    )
                    .unwrap();
                    Self::Orchard(orchard::Note::from_parts(recipient, value, rho, rseed).unwrap())
                }
                _ => panic!("invalid protocol"),
            }
        }
    }

    #[cfg(test)]
    mod tests {
        use super::*;
        use crate::proto::memwallet as proto;
        use pretty_assertions::assert_eq;

        #[test]
        fn test_note_roundtrip() {
            let note = Note::Sapling(sapling::note::Note::from_parts(
                sapling::PaymentAddress::from_bytes(&[
                    0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x30, 0x8e,
                    0x11, 0x9d, 0x72, 0x99, 0x2b, 0x56, 0x0d, 0x26, 0x50, 0xff, 0xe0, 0xbe, 0x7f,
                    0x35, 0x42, 0xfd, 0x97, 0x00, 0x3c, 0xb7, 0xcc, 0x3a, 0xbf, 0xf8, 0x1a, 0x7f,
                    0x90, 0x37, 0xf3, 0xea,
                ])
                .unwrap(),
                sapling::value::NoteValue::from_raw(99),
                sapling::Rseed::AfterZip212([0; 32]),
            ));

            let proto_note: proto::Note = note.clone().into();
            let recovered: Note = proto_note.into();

            assert_eq!(note, recovered);
        }
    }
}
