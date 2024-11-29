use std::{collections::VecDeque, ops::Deref};

use zcash_client_backend::data_api::TransactionDataRequest;
use zcash_primitives::transaction::TxId;
#[derive(Debug, Default, PartialEq)]
pub struct TransactionDataRequestQueue(pub(crate) VecDeque<TransactionDataRequest>);

impl TransactionDataRequestQueue {
    pub fn new() -> Self {
        Self(VecDeque::new())
    }

    pub fn queue_status_retrieval(&mut self, txid: &TxId) {
        self.0.push_back(TransactionDataRequest::GetStatus(*txid));
    }
}

impl Deref for TransactionDataRequestQueue {
    type Target = VecDeque<TransactionDataRequest>;

    fn deref(&self) -> &Self::Target {
        &self.0
    }
}

mod serialization {
    use super::*;
    use crate::proto::memwallet as proto;
    use zcash_keys::encoding::AddressCodec;
    use zcash_primitives::{
        consensus::Network::MainNetwork as EncodingParams, legacy::TransparentAddress,
    };

    impl From<TransactionDataRequest> for proto::TransactionDataRequest {
        fn from(request: TransactionDataRequest) -> Self {
            match request {
                TransactionDataRequest::GetStatus(txid) => Self {
                    request_type: proto::TransactionDataRequestType::GetStatus as i32,
                    tx_id: Some(txid.into()),
                    address: None,
                    block_range_start: None,
                    block_range_end: None,
                },
                TransactionDataRequest::Enhancement(txid) => Self {
                    request_type: proto::TransactionDataRequestType::Enhancement as i32,
                    tx_id: Some(txid.into()),
                    address: None,
                    block_range_start: None,
                    block_range_end: None,
                },
                TransactionDataRequest::SpendsFromAddress {
                    address,
                    block_range_start,
                    block_range_end,
                } => Self {
                    request_type: proto::TransactionDataRequestType::SpendsFromAddress as i32,
                    tx_id: None,
                    address: Some(address.encode(&EncodingParams).as_bytes().to_vec()),
                    block_range_start: Some(block_range_start.into()),
                    block_range_end: block_range_end.map(Into::into),
                },
            }
        }
    }

    impl From<proto::TransactionDataRequest> for TransactionDataRequest {
        fn from(request: proto::TransactionDataRequest) -> Self {
            match request.request_type {
                0 => TransactionDataRequest::GetStatus(request.tx_id.unwrap().into()),
                1 => TransactionDataRequest::Enhancement(request.tx_id.unwrap().into()),
                2 => TransactionDataRequest::SpendsFromAddress {
                    address: TransparentAddress::decode(
                        &EncodingParams,
                        &String::from_utf8(request.address.unwrap()).unwrap(),
                    )
                    .unwrap(),
                    block_range_start: request.block_range_start.unwrap().into(),
                    block_range_end: request.block_range_end.map(Into::into),
                },
                _ => panic!("invalid request type"),
            }
        }
    }
}
