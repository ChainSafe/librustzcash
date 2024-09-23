use std::collections::BTreeMap;

use zcash_client_backend::data_api::InputSource;
use zcash_client_backend::wallet::Note;
use zcash_client_backend::wallet::WalletTransparentOutput;
use zcash_client_backend::{
    data_api::{
        chain::BlockSource,
        testing::{DataStoreFactory, Reset, TestCache, TestState, TransactionSummary},
        WalletRead, WalletTest,
    },
    proto::compact_formats::CompactBlock,
};
use zcash_protocol::ShieldedProtocol;

use shardtree::store::ShardStore;
use zcash_client_backend::wallet::NoteId;
use zcash_client_backend::wallet::ReceivedNote;
use zcash_primitives::transaction::components::OutPoint;
use zcash_primitives::transaction::TxId;
use zcash_protocol::consensus::BlockHeight;
use zcash_protocol::local_consensus::LocalNetwork;

use crate::{Account, AccountId, Error, MemoryWalletDb};

pub mod pool;

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

/// A block cache for testing. Just holds blocks in a map
pub(crate) struct MemBlockCache(BTreeMap<u64, CompactBlock>);

impl MemBlockCache {
    pub(crate) fn new() -> Self {
        MemBlockCache(BTreeMap::new())
    }
}

impl BlockSource for MemBlockCache {
    type Error = ();

    fn with_blocks<F, WalletErrT>(
        &self,
        from_height: Option<zcash_protocol::consensus::BlockHeight>,
        limit: Option<usize>,
        mut with_block: F,
    ) -> Result<(), zcash_client_backend::data_api::chain::error::Error<WalletErrT, Self::Error>>
    where
        F: FnMut(
            CompactBlock,
        ) -> Result<
            (),
            zcash_client_backend::data_api::chain::error::Error<WalletErrT, Self::Error>,
        >,
    {
        let block_iter = self
            .0
            .iter()
            .filter(|(_, cb)| {
                if let Some(from_height) = from_height {
                    cb.height() >= from_height
                } else {
                    true
                }
            })
            .take(limit.unwrap_or(usize::MAX));

        for (_, cb) in block_iter {
            with_block(cb.clone())?;
        }
        Ok(())
    }
}

impl TestCache for MemBlockCache {
    type BsError = ();
    type BlockSource = MemBlockCache;
    type InsertResult = ();

    fn block_source(&self) -> &Self::BlockSource {
        self
    }

    fn insert(&mut self, cb: &CompactBlock) -> Self::InsertResult {
        self.0.insert(cb.height().into(), cb.clone());
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
    fn get_confirmed_sends(
        &self,
        txid: &TxId,
    ) -> Result<Vec<(u64, Option<String>, Option<String>, Option<u32>)>, <Self as WalletRead>::Error>
    {
        todo!()
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
        todo!()
    }

    #[cfg(any(test, feature = "test-dependencies"))]
    fn get_notes(
        &self,
        protocol: zcash_protocol::ShieldedProtocol,
    ) -> Result<Vec<ReceivedNote<Self::NoteRef, Note>>, <Self as InputSource>::Error> {
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
    #[cfg(any(test, feature = "test-dependencies"))]
    fn get_sent_note_ids(
        &self,
        txid: &TxId,
        protocol: ShieldedProtocol,
    ) -> Result<Vec<NoteId>, <Self as WalletRead>::Error> {
        Ok(self
            .get_sent_notes()
            .iter()
            .filter_map(|(id, _)| {
                if id.txid() == txid && id.protocol() == protocol {
                    Some(*id)
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
    #[cfg(any(test, feature = "test-dependencies"))]
    fn get_tx_history(
        &self,
    ) -> Result<
        Vec<TransactionSummary<<Self as WalletRead>::AccountId>>,
        <Self as InputSource>::Error,
    > {
        // TODO: This is only looking at sent notes, we need to look at received notes as well
        // TODO: Need to actually implement a bunch of these fields
        Ok(self
            .sent_notes
            .iter()
            .map(|(note_id, note)| {
                zcash_client_backend::data_api::testing::TransactionSummary::from_parts(
                    note.from_account_id,  // account_id
                    *note_id.txid(),       // txid
                    None,                  // expiry_height
                    None,                  // mined_height
                    0.try_into().unwrap(), // account_value_delta
                    None,                  // fee_paid
                    0,                     // spent_note_count
                    false,                 // has_change
                    0,                     // sent_note_count
                    0,                     // received_note_count
                    0,                     // memo_count
                    false,                 // expired_unmined
                    false,                 // is_shielding
                )
            })
            .collect::<Vec<_>>())
    }

    #[cfg(any(test, feature = "test-dependencies"))]
    fn get_checkpoint_history(
        &self,
    ) -> Result<
        Vec<(
            BlockHeight,
            ShieldedProtocol,
            Option<incrementalmerkletree::Position>,
        )>,
        <Self as InputSource>::Error,
    > {
        let mut checkpoints = Vec::new();

        self.sapling_tree
            .store()
            .for_each_checkpoint(usize::MAX, |id, cp| {
                checkpoints.push((id.clone(), ShieldedProtocol::Sapling, cp.position()));
                Ok(())
            })?;

        #[cfg(feature = "orchard")]
        self.orchard_tree
            .store()
            .for_each_checkpoint(usize::MAX, |id, cp| {
                checkpoints.push((id.clone(), ShieldedProtocol::Orchard, cp.position()));
                Ok(())
            })?;

        checkpoints.sort_by(|(a, _, _), (b, _, _)| a.cmp(b));

        Ok(checkpoints)
    }
}
