use std::collections::BTreeMap;

use serde::{Deserialize, Serialize};
use serde_with::serde_as;
use serde_with::FromInto;
use zcash_client_backend::wallet::WalletTransparentOutput;
use zcash_primitives::{
    legacy::TransparentAddress,
    transaction::{
        components::{OutPoint, TxOut},
        TxId,
    },
};
use zcash_protocol::consensus::BlockHeight;

use super::AccountId;
use crate::{ByteArray, OutPointDef, TransparentAddressDef, TxOutDef};

pub struct TransparentOutputTable(BTreeMap<OutPoint, ReceivedTransparentUtxo>);

impl TransparentOutputTable {
    pub fn new() -> Self {
        Self(BTreeMap::new())
    }
}

#[serde_as]
#[derive(Serialize, Deserialize)]
pub struct ReceivedTransparentUtxoSpends(
    #[serde_as(as = "BTreeMap<OutPointDef, ByteArray<32>>")] BTreeMap<OutPoint, TxId>,
);

#[serde_as]
#[derive(Serialize, Deserialize)]
// transparent_received_outputs
pub struct ReceivedTransparentUtxo {
    // transaction_id, output_index
    #[serde_as(as = "OutPointDef")]
    outpoint: OutPoint,
    // Spend authority of this utxo
    account_id: AccountId,
    // value_zat, script_pubkey
    #[serde_as(as = "TxOutDef")]
    txout: TxOut,
    #[serde_as(as = "TransparentAddressDef")]
    recipient_address: TransparentAddress,
    #[serde_as(as = "Option<FromInto<u32>>")]
    max_observed_unspent_height: Option<BlockHeight>,
    // ??? do we need?
    #[serde_as(as = "Option<FromInto<u32>>")]
    mined_height: Option<BlockHeight>,
}

#[serde_as]
#[derive(Serialize, Deserialize)]
pub struct SentTransparentUtxoTable(
    #[serde_as(as = "BTreeMap<OutPointDef,_>")] BTreeMap<OutPoint, SentUtxo>,
);
#[derive(Serialize, Deserialize)]
struct SentUtxo {}
