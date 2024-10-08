use std::collections::BTreeMap;
use std::convert::Infallible;

use zcash_address::ZcashAddress;
use zcash_client_backend::data_api::InputSource;
use zcash_client_backend::data_api::OutputOfSentTx;
use zcash_client_backend::wallet::Note;
use zcash_client_backend::wallet::Recipient;
use zcash_client_backend::wallet::WalletTransparentOutput;
use zcash_client_backend::{
    data_api::{
        chain::BlockSource,
        testing::{DataStoreFactory, Reset, TestCache, TestState, TransactionSummary},
        WalletRead, WalletTest,
    },
    proto::compact_formats::CompactBlock,
};
use zcash_keys::address::Address;
use zcash_primitives::transaction::components::amount::NonNegativeAmount;
use zcash_protocol::value::ZatBalance;
use zcash_protocol::ShieldedProtocol;

use shardtree::store::ShardStore;
use zcash_client_backend::wallet::NoteId;
use zcash_client_backend::wallet::ReceivedNote;
use zcash_primitives::transaction::components::OutPoint;
use zcash_primitives::transaction::TxId;
use zcash_protocol::consensus::BlockHeight;
use zcash_protocol::local_consensus::LocalNetwork;

use crate::{Account, AccountId, Error, MemBlockCache, MemoryWalletDb, SentNoteId};

pub mod pool;

#[cfg(test)]
#[cfg(feature = "transparent-inputs")]
mod transparent;

/// A test data store factory for in-memory databases
/// Very simple implementation just creates a new MemoryWalletDb
pub(crate) struct TestMemDbFactory;

impl DataStoreFactory for TestMemDbFactory {
    type Error = ();
    type AccountId = AccountId;
    type Account = Account;
    type DsError = Error;
    type DataStore = MemoryWalletDb<LocalNetwork>;

    fn new_data_store(&self, network: LocalNetwork) -> Result<Self::DataStore, Self::Error> {
        Ok(MemoryWalletDb::new(network, 100))
    }
}

impl TestCache for MemBlockCache {
    type BsError = Infallible;
    type BlockSource = MemBlockCache;
    type InsertResult = ();

    fn block_source(&self) -> &Self::BlockSource {
        self
    }

    fn insert(&mut self, cb: &CompactBlock) -> Self::InsertResult {
        self.0
            .write()
            .unwrap()
            .insert(cb.height().into(), cb.clone());
    }

    fn truncate_to_height(&mut self, height: BlockHeight) {
        self.0.write().unwrap().retain(|k, _| *k <= height);
    }
}

impl<P> Reset for MemoryWalletDb<P>
where
    P: zcash_primitives::consensus::Parameters + Clone,
{
    type Handle = ();

    fn reset<C>(st: &mut TestState<C, Self, LocalNetwork>) -> Self::Handle {
        let new_wallet = MemoryWalletDb::new(st.wallet().params.clone(), 100);
        let _ = std::mem::replace(st.wallet_mut(), new_wallet);
    }
}

impl<P> WalletTest for MemoryWalletDb<P>
where
    P: zcash_primitives::consensus::Parameters + Clone,
{
    #[allow(clippy::type_complexity)]
    fn get_sent_outputs(&self, txid: &TxId) -> Result<Vec<OutputOfSentTx>, Error> {
        Ok(self
            .sent_notes
            .iter()
            .filter(|(note_id, _)| note_id.txid() == txid)
            .map(|(_, note)| match note.to.clone() {
                Recipient::External(zcash_address, _) => Ok((
                    note.value.into_u64(),
                    Some(
                        Address::try_from_zcash_address(&self.params, zcash_address)
                            .map_err(Error::from)?,
                    ),
                    None,
                )),
                Recipient::EphemeralTransparent {
                    ephemeral_address,
                    receiving_account,
                    ..
                } => {
                    let account = self.get_account(receiving_account)?.unwrap();
                    let idx = *account
                        .addresses()
                        .iter()
                        .find(|(_, addr)| addr.transparent() == Some(&ephemeral_address))
                        .unwrap()
                        .0;
                    let idx = idx.try_into().unwrap();

                    Ok((
                        // TODO: Use the ephemeral address index to look up the address
                        // and find the correct index
                        note.value.into_u64(),
                        Some(Address::from(ephemeral_address)),
                        Some((Address::from(ephemeral_address), idx)),
                    ))
                }
                Recipient::InternalAccount { .. } => Ok((note.value.into_u64(), None, None)),
            })
            .map(|res: Result<_, Error>| {
                let (amount, external_recipient, ephemeral_address) = res?;
                Ok::<_, <Self as WalletRead>::Error>(OutputOfSentTx::from_parts(
                    NonNegativeAmount::from_u64(amount)?,
                    external_recipient,
                    ephemeral_address,
                ))
            })
            .collect::<Result<_, Error>>()?)
    }

    #[doc = " Fetches the transparent output corresponding to the provided `outpoint`."]
    #[doc = " Allows selecting unspendable outputs for testing purposes."]
    #[doc = ""]
    #[doc = " Returns `Ok(None)` if the UTXO is not known to belong to the wallet or is not"]
    #[doc = " spendable as of the chain tip height."]
    #[cfg(feature = "transparent-inputs")]
    fn get_transparent_output(
        &self,
        outpoint: &zcash_primitives::transaction::components::OutPoint,
        allow_unspendable: bool,
    ) -> Result<Option<WalletTransparentOutput>, <Self as InputSource>::Error> {
        Ok(self
            .transparent_received_outputs
            .get(outpoint)
            .map(|txo| (txo, self.tx_table.get(&txo.transaction_id)))
            .map(|(txo, tx)| {
                txo.to_wallet_transparent_output(outpoint, tx.map(|tx| tx.mined_height()).flatten())
            })
            .flatten())
    }

    fn get_notes(
        &self,
        protocol: zcash_protocol::ShieldedProtocol,
    ) -> Result<Vec<ReceivedNote<Self::NoteRef, Note>>, Error> {
        Ok(self
            .received_notes
            .iter()
            .filter(|rn| rn.note.protocol() == protocol)
            .cloned()
            .map(Into::into)
            .collect())
    }

    /// Returns the note IDs for shielded notes sent by the wallet in a particular
    /// transaction.
    fn get_sent_note_ids(
        &self,
        txid: &TxId,
        protocol: ShieldedProtocol,
    ) -> Result<Vec<NoteId>, Error> {
        Ok(self
            .get_sent_notes()
            .iter()
            .filter_map(|(id, _)| {
                if let SentNoteId::Shielded(id) = id {
                    if id.txid() == txid && id.protocol() == protocol {
                        Some(*id)
                    } else {
                        None
                    }
                } else {
                    None
                }
            })
            .collect())
    }

    /// Returns a vector of transaction summaries.
    ///
    /// Currently test-only, as production use could return a very large number of results; either
    /// pagination or a streaming design will be necessary to stabilize this feature for production
    /// use.⁄
    fn get_tx_history(
        &self,
    ) -> Result<Vec<zcash_client_backend::data_api::testing::TransactionSummary<AccountId>>, Error>
    {
        Ok(self
            .tx_table
            .iter()
            .map(|(txid, tx)| {
                // find all the notes associated with this transaction

                // notes spent by the transaction
                let spent_notes = self
                    .received_note_spends
                    .iter()
                    .filter(|(_, spend_txid)| *spend_txid == txid)
                    .collect::<Vec<_>>();

                let spent_utxos = self
                    .transparent_received_output_spends
                    .iter()
                    .filter(|(_, spend_txid)| *spend_txid == txid)
                    .collect::<Vec<_>>();

                // notes produced (sent) by the transaction (excluding change)
                let sent_notes = self
                    .sent_notes
                    .iter()
                    .filter(|(note_id, _)| note_id.txid() == txid)
                    .filter(|(note_id, _)| {
                        // use a join on the received notes table to detect which are change
                        self.received_notes.iter().any(|received_note| {
                            SentNoteId::from(received_note.note_id) == **note_id
                                && !received_note.is_change
                        })
                    })
                    .collect::<Vec<_>>();
                let received_txo = self
                    .transparent_received_outputs
                    .iter()
                    .filter(|(outpoint, received_output)| outpoint.txid() == txid)
                    .collect::<Vec<_>>();
                let sent_txo_value: u64 = received_txo
                    .iter()
                    .map(|(_, o)| u64::from(o.txout.value))
                    .sum();
                // notes received by the transaction
                let received_notes = self
                    .received_notes
                    .iter()
                    .filter(|received_note| received_note.txid() == *txid)
                    .collect::<Vec<_>>();

                let account_id = sent_notes
                    .first()
                    .map(|(_, note)| note.from_account_id)
                    .unwrap_or_default();

                let balance_gained: u64 = received_notes
                    .iter()
                    .map(|note| note.note.value().into_u64())
                    .sum::<u64>()
                    + sent_txo_value;

                let balance_lost: u64 = self // includes change
                    .sent_notes
                    .iter()
                    .filter(|(note_id, _)| note_id.txid() == txid)
                    .map(|(_, sent_note)| sent_note.value.into_u64())
                    .sum::<u64>()
                    + tx.fee().map(u64::from).unwrap_or(0);

                println!(
                    "balance_gained:{}, sent_txo_value: {}, balance_lost:{}",
                    balance_gained, sent_txo_value, balance_lost
                );
                let is_shielding = {
                    //All of the wallet-spent and wallet-received notes are consistent with a shielding transaction.
                    // e.g. only transparent outputs are spend and only shielded notes are received
                    spent_notes.is_empty() && !spent_utxos.is_empty()
                        // The transaction contains at least one wallet-received note.
                        && !received_notes.is_empty()
                        // We do not know about any external outputs of the transaction.
                        && sent_notes.is_empty()
                };

                zcash_client_backend::data_api::testing::TransactionSummary::from_parts(
                    account_id,                                                                  // account_id
                    *txid,              // txid
                    tx.expiry_height(), // expiry_height
                    tx.mined_height(),  // mined_height
                    ZatBalance::const_from_i64((balance_gained as i64) - (balance_lost as i64)), // account_value_delta
                    tx.fee(),                                         // fee_paid
                    spent_notes.len() + spent_utxos.len(),            // spent_note_count
                    received_notes.iter().any(|note| note.is_change), // has_change
                    sent_notes.len(), // sent_note_count (excluding change)
                    received_notes.iter().filter(|note| !note.is_change).count()
                        + received_txo.len(), // received_note_count (excluding change)
                    0,                // TODO: memo_count
                    false,            // TODO: expired_unmined
                    is_shielding,     // is_shielding
                )
            })
            .collect())
    }

    fn get_checkpoint_history(
        &self,
        protocol: &ShieldedProtocol,
    ) -> Result<Vec<(BlockHeight, Option<incrementalmerkletree::Position>)>, Error> {
        let mut checkpoints = Vec::new();

        match protocol {
            ShieldedProtocol::Sapling => {
                self.sapling_tree
                    .store()
                    .for_each_checkpoint(usize::MAX, |id, cp| {
                        checkpoints.push((id.clone(), cp.position()));
                        Ok(())
                    })?;
            }
            #[cfg(feature = "orchard")]
            ShieldedProtocol::Orchard => {
                self.orchard_tree
                    .store()
                    .for_each_checkpoint(usize::MAX, |id, cp| {
                        checkpoints.push((id.clone(), cp.position()));
                        Ok(())
                    })?;
            }
            _ => {}
        }

        checkpoints.sort_by(|(a, _), (b, _)| a.cmp(b));

        Ok(checkpoints)
    }
}
