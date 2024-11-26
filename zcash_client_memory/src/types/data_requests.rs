use std::{collections::VecDeque, ops::Deref};

use zcash_client_backend::data_api::TransactionDataRequest;
use zcash_primitives::transaction::TxId;
#[derive(Default)]
pub struct TransactionDataRequestQueue(VecDeque<TransactionDataRequest>);

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
