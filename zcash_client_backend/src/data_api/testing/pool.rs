use assert_matches::assert_matches;
use incrementalmerkletree::{frontier::Frontier, Level};
use rand::RngCore;
use secrecy::Secret;
use shardtree::error::ShardTreeError;
use std::{
    cmp::Eq,
    convert::Infallible,
    hash::Hash,
    num::{NonZeroU32, NonZeroU8},
};
use zip32::Scope;

use zcash_keys::{address::Address, keys::UnifiedSpendingKey};
use zcash_primitives::{
    block::BlockHash,
    legacy::TransparentAddress,
    transaction::{
        components::amount::NonNegativeAmount,
        fees::{fixed::FeeRule as FixedFeeRule, StandardFeeRule},
        Transaction,
    },
};
use zcash_protocol::{
    consensus::{self, BlockHeight, NetworkUpgrade, Parameters},
    memo::{Memo, MemoBytes},
    value::Zatoshis,
    ShieldedProtocol,
};
use zip321::{Payment, TransactionRequest};

use crate::{
    data_api::{
        self,
        chain::{ChainState, CommitmentTreeRoot, ScanSummary},
        testing::{input_selector, AddressType, FakeCompactOutput, InitialChainState, TestBuilder},
        wallet::{decrypt_and_store_transaction, input_selection::GreedyInputSelector},
        Account as _, AccountBirthday, DecryptedTransaction, InputSource, Ratio,
        WalletCommitmentTrees, WalletRead, WalletSummary, WalletWrite,
    },
    decrypt_transaction,
    fees::{fixed::SingleOutputChangeStrategy, standard, DustOutputPolicy},
    wallet::{Note, NoteId, OvkPolicy, ReceivedNote},
};

use super::{DataStoreFactory, Reset, TestCache, TestFvk, TestState};

/// The number of ephemeral addresses that can be safely reserved without observing any
/// of them to be mined. This is the same as the gap limit in Bitcoin.
#[cfg(feature = "transparent-inputs")]
pub(crate) const GAP_LIMIT: u32 = 20;

/// Trait that exposes the pool-specific types and operations necessary to run the
/// single-shielded-pool tests on a given pool.
pub trait ShieldedPoolTester {
    const SHIELDED_PROTOCOL: ShieldedProtocol;

    type Sk;
    type Fvk: TestFvk;
    type MerkleTreeHash;
    type Note;

    fn test_account_fvk<Cache, DbT: WalletRead, P: consensus::Parameters>(
        st: &TestState<Cache, DbT, P>,
    ) -> Self::Fvk;
    fn usk_to_sk(usk: &UnifiedSpendingKey) -> &Self::Sk;
    fn sk(seed: &[u8]) -> Self::Sk;
    fn sk_to_fvk(sk: &Self::Sk) -> Self::Fvk;
    fn sk_default_address(sk: &Self::Sk) -> Address;
    fn fvk_default_address(fvk: &Self::Fvk) -> Address;
    fn fvks_equal(a: &Self::Fvk, b: &Self::Fvk) -> bool;

    fn random_fvk(mut rng: impl RngCore) -> Self::Fvk {
        let sk = {
            let mut sk_bytes = vec![0; 32];
            rng.fill_bytes(&mut sk_bytes);
            Self::sk(&sk_bytes)
        };

        Self::sk_to_fvk(&sk)
    }
    fn random_address(rng: impl RngCore) -> Address {
        Self::fvk_default_address(&Self::random_fvk(rng))
    }

    fn empty_tree_leaf() -> Self::MerkleTreeHash;
    fn empty_tree_root(level: Level) -> Self::MerkleTreeHash;

    fn put_subtree_roots<Cache, DbT: WalletRead + WalletCommitmentTrees, P>(
        st: &mut TestState<Cache, DbT, P>,
        start_index: u64,
        roots: &[CommitmentTreeRoot<Self::MerkleTreeHash>],
    ) -> Result<(), ShardTreeError<<DbT as WalletCommitmentTrees>::Error>>;

    fn next_subtree_index<A: Hash + Eq>(s: &WalletSummary<A>) -> u64;

    #[allow(clippy::type_complexity)]
    fn select_spendable_notes<Cache, DbT: InputSource + WalletRead, P>(
        st: &TestState<Cache, DbT, P>,
        account: <DbT as InputSource>::AccountId,
        target_value: Zatoshis,
        anchor_height: BlockHeight,
        exclude: &[DbT::NoteRef],
    ) -> Result<Vec<ReceivedNote<DbT::NoteRef, Self::Note>>, <DbT as InputSource>::Error>;

    fn decrypted_pool_outputs_count<A>(d_tx: &DecryptedTransaction<'_, A>) -> usize;

    fn with_decrypted_pool_memos<A>(d_tx: &DecryptedTransaction<'_, A>, f: impl FnMut(&MemoBytes));

    fn try_output_recovery<P: consensus::Parameters>(
        params: &P,
        height: BlockHeight,
        tx: &Transaction,
        fvk: &Self::Fvk,
    ) -> Option<(Note, Address, MemoBytes)>;

    fn received_note_count(summary: &ScanSummary) -> usize;
}

pub fn send_single_step_proposed_transfer<T: ShieldedPoolTester>(
    dsf: impl DataStoreFactory,
    cache: impl TestCache,
) {
    let mut st = TestBuilder::new()
        .with_data_store_factory(dsf)
        .with_block_cache(cache)
        .with_account_from_sapling_activation(BlockHash([0; 32]))
        .build();

    let account = st.test_account().cloned().unwrap();
    let dfvk = T::test_account_fvk(&st);

    // Add funds to the wallet in a single note
    let value = Zatoshis::const_from_u64(60000);
    let (h, _, _) = st.generate_next_block(&dfvk, AddressType::DefaultExternal, value);
    st.scan_cached_blocks(h, 1);

    // Spendable balance matches total balance
    assert_eq!(st.get_total_balance(account.id()), value);
    assert_eq!(st.get_spendable_balance(account.id(), 1), value);

    assert_eq!(
        st.wallet()
            .block_max_scanned()
            .unwrap()
            .unwrap()
            .block_height(),
        h
    );

    let to_extsk = T::sk(&[0xf5; 32]);
    let to: Address = T::sk_default_address(&to_extsk);
    let request = zip321::TransactionRequest::new(vec![Payment::without_memo(
        to.to_zcash_address(st.network()),
        Zatoshis::const_from_u64(10000),
    )])
    .unwrap();

    let fee_rule = StandardFeeRule::Zip317;

    let change_memo = "Test change memo".parse::<Memo>().unwrap();
    let change_strategy = standard::SingleOutputChangeStrategy::new(
        fee_rule,
        Some(change_memo.clone().into()),
        T::SHIELDED_PROTOCOL,
    );
    let input_selector = &GreedyInputSelector::new(change_strategy, DustOutputPolicy::default());

    let proposal = st
        .propose_transfer(
            account.id(),
            input_selector,
            request,
            NonZeroU32::new(1).unwrap(),
        )
        .unwrap();

    let create_proposed_result = st.create_proposed_transactions::<Infallible, _>(
        account.usk(),
        OvkPolicy::Sender,
        &proposal,
    );
    assert_matches!(&create_proposed_result, Ok(txids) if txids.len() == 1);

    let sent_tx_id = create_proposed_result.unwrap()[0];

    // Verify that the sent transaction was stored and that we can decrypt the memos
    let tx = st
        .wallet()
        .get_transaction(sent_tx_id)
        .unwrap()
        .expect("Created transaction was stored.");
    let ufvks = [(account.id(), account.usk().to_unified_full_viewing_key())]
        .into_iter()
        .collect();
    let d_tx = decrypt_transaction(st.network(), h + 1, &tx, &ufvks);
    assert_eq!(T::decrypted_pool_outputs_count(&d_tx), 2);

    let mut found_tx_change_memo = false;
    let mut found_tx_empty_memo = false;
    T::with_decrypted_pool_memos(&d_tx, |memo| {
        if Memo::try_from(memo).unwrap() == change_memo {
            found_tx_change_memo = true
        }
        if Memo::try_from(memo).unwrap() == Memo::Empty {
            found_tx_empty_memo = true
        }
    });
    assert!(found_tx_change_memo);
    assert!(found_tx_empty_memo);

    // Verify that the stored sent notes match what we're expecting
    let sent_note_ids = st
        .wallet()
        .get_sent_note_ids(&sent_tx_id, T::SHIELDED_PROTOCOL)
        .unwrap();
    assert_eq!(sent_note_ids.len(), 2);

    // The sent memo should be the empty memo for the sent output, and the
    // change output's memo should be as specified.
    let mut found_sent_change_memo = false;
    let mut found_sent_empty_memo = false;
    for sent_note_id in sent_note_ids {
        match st
            .wallet()
            .get_memo(sent_note_id)
            .expect("Note id is valid")
            .as_ref()
        {
            Some(m) if m == &change_memo => {
                found_sent_change_memo = true;
            }
            Some(m) if m == &Memo::Empty => {
                found_sent_empty_memo = true;
            }
            Some(other) => panic!("Unexpected memo value: {:?}", other),
            None => panic!("Memo should not be stored as NULL"),
        }
    }
    assert!(found_sent_change_memo);
    assert!(found_sent_empty_memo);

    // Check that querying for a nonexistent sent note returns None
    assert_matches!(
        st.wallet()
            .get_memo(NoteId::new(sent_tx_id, T::SHIELDED_PROTOCOL, 12345)),
        Ok(None)
    );

    let tx_history = st.wallet().get_tx_history().unwrap();
    assert_eq!(tx_history.len(), 2);

    let network = *st.network();
    assert_matches!(
        decrypt_and_store_transaction(&network, st.wallet_mut(), &tx, None),
        Ok(_)
    );
}

#[cfg(feature = "transparent-inputs")]
pub fn send_multi_step_proposed_transfer<T: ShieldedPoolTester, DSF>(
    dsf: DSF,
    cache: impl TestCache,
) where
    DSF: DataStoreFactory,
    <DSF as DataStoreFactory>::AccountId: std::fmt::Debug,
{
    use std::{collections::HashSet, str::FromStr};

    use crate::{
        data_api::{TransactionDataRequest, TransactionStatus},
        fees::ChangeValue,
        wallet::{TransparentAddressMetadata, WalletTransparentOutput},
    };
    use rand_core::OsRng;
    use zcash_primitives::{
        legacy::keys::{NonHardenedChildIndex, TransparentKeyScope},
        transaction::{
            builder::{BuildConfig, Builder},
            components::{OutPoint, TxOut},
            fees::zip317,
        },
    };
    use zcash_proofs::prover::LocalTxProver;
    use zcash_protocol::value::ZatBalance;

    let mut st = TestBuilder::new()
        .with_data_store_factory(dsf)
        .with_block_cache(cache)
        .with_account_from_sapling_activation(BlockHash([0; 32]))
        .build();

    let account = st.test_account().cloned().unwrap();
    let account_id = account.id();
    let (default_addr, default_index) = account.usk().default_transparent_address();
    let dfvk = T::test_account_fvk(&st);

    let value = NonNegativeAmount::const_from_u64(100000);
    let transfer_amount = NonNegativeAmount::const_from_u64(50000);

    let run_test = |st: &mut TestState<_, DSF::DataStore, _>, expected_index| {
        // Add funds to the wallet.
        st.add_funds(&account, &dfvk, value);

        let expected_step0_fee = (zip317::MARGINAL_FEE * 3).unwrap();
        let expected_step1_fee = zip317::MINIMUM_FEE;
        let expected_ephemeral = (transfer_amount + expected_step1_fee).unwrap();
        let expected_step0_change =
            (value - expected_ephemeral - expected_step0_fee).expect("sufficient funds");
        assert!(expected_step0_change.is_positive());

        // Generate a ZIP 320 proposal, sending to the wallet's default transparent address
        // expressed as a TEX address.
        let tex_addr = match default_addr {
            TransparentAddress::PublicKeyHash(data) => Address::Tex(data),
            _ => unreachable!(),
        };
        let change_memo = Some(Memo::from_str("change").expect("valid memo").encode());

        // We use `st.propose_standard_transfer` here in order to also test round-trip
        // serialization of the proposal.
        let proposal = st
            .propose_standard_transfer::<Infallible>(
                account_id,
                StandardFeeRule::Zip317,
                NonZeroU32::new(1).unwrap(),
                &tex_addr,
                transfer_amount,
                None,
                change_memo.clone(),
                T::SHIELDED_PROTOCOL,
            )
            .unwrap();

        let steps: Vec<_> = proposal.steps().iter().cloned().collect();
        assert_eq!(steps.len(), 2);

        assert_eq!(steps[0].balance().fee_required(), expected_step0_fee);
        assert_eq!(steps[1].balance().fee_required(), expected_step1_fee);
        assert_eq!(
            steps[0].balance().proposed_change(),
            [
                ChangeValue::shielded(T::SHIELDED_PROTOCOL, expected_step0_change, change_memo),
                ChangeValue::ephemeral_transparent(expected_ephemeral),
            ]
        );
        assert_eq!(steps[1].balance().proposed_change(), []);

        let create_proposed_result = st.create_proposed_transactions::<Infallible, _>(
            account.usk(),
            OvkPolicy::Sender,
            &proposal,
        );
        assert_matches!(&create_proposed_result, Ok(txids) if txids.len() == 2);
        let txids = create_proposed_result.unwrap();

        // Check that there are sent outputs with the correct values.
        let confirmed_sent: Vec<Vec<_>> = txids
            .iter()
            .map(|sent_txid| st.wallet().get_confirmed_sends(sent_txid).unwrap())
            .collect();

        // Verify that a status request has been generated for the second transaction of
        // the ZIP 320 pair.
        let tx_data_requests = st.wallet().transaction_data_requests().unwrap();
        assert!(tx_data_requests.contains(&TransactionDataRequest::GetStatus(*txids.last())));

        assert!(expected_step0_change < expected_ephemeral);
        assert_eq!(confirmed_sent.len(), 2);
        assert_eq!(confirmed_sent[0].len(), 2);
        assert_eq!(
            confirmed_sent[0][0].0,
            u64::try_from(expected_step0_change).unwrap()
        );
        let (ephemeral_v, to_addr, ephemeral_addr, index) = confirmed_sent[0][1].clone();
        assert_eq!(ephemeral_v, u64::try_from(expected_ephemeral).unwrap());
        assert!(to_addr.is_some());
        assert_eq!(ephemeral_addr, to_addr);
        assert_eq!(index, Some(expected_index));

        assert_eq!(confirmed_sent[1].len(), 1);
        assert_matches!(
            confirmed_sent[1][0].clone(),
            (sent_v, sent_to_addr, None, None)
            if sent_v == u64::try_from(transfer_amount).unwrap() && sent_to_addr == Some(tex_addr.encode(st.network())));

        // Check that the transaction history matches what we expect.
        let tx_history = st.wallet().get_tx_history().unwrap();

        let tx_0 = tx_history
            .iter()
            .find(|tx| tx.txid() == *txids.first())
            .unwrap();
        let tx_1 = tx_history
            .iter()
            .find(|tx| tx.txid() == *txids.last())
            .unwrap();

        assert_eq!(tx_0.account_id(), &account_id);
        assert!(!tx_0.expired_unmined());
        assert_eq!(tx_0.has_change(), expected_step0_change.is_positive());
        assert!(!tx_0.is_shielding());
        assert_eq!(
            tx_0.account_value_delta(),
            -ZatBalance::from(expected_step0_fee),
        );

        assert_eq!(tx_1.account_id(), &account_id);
        assert!(!tx_1.expired_unmined());
        assert!(!tx_1.has_change());
        assert!(!tx_0.is_shielding());
        assert_eq!(
            tx_1.account_value_delta(),
            -ZatBalance::from(expected_ephemeral),
        );

        (ephemeral_addr.unwrap(), txids)
    };

    // Each transfer should use a different ephemeral address.
    let (ephemeral0, txids0) = run_test(&mut st, 0);
    let (ephemeral1, txids1) = run_test(&mut st, 1);
    assert_ne!(ephemeral0, ephemeral1);

    let height = st.add_funds(&account, &dfvk, value);

    let ephemeral_taddr = Address::decode(st.network(), &ephemeral0).expect("valid address");
    assert_matches!(
        ephemeral_taddr,
        Address::Transparent(TransparentAddress::PublicKeyHash(_))
    );

    // Attempting to pay to an ephemeral address should cause an error.
    let proposal = st
        .propose_standard_transfer::<Infallible>(
            account_id,
            StandardFeeRule::Zip317,
            NonZeroU32::new(1).unwrap(),
            &ephemeral_taddr,
            transfer_amount,
            None,
            None,
            T::SHIELDED_PROTOCOL,
        )
        .unwrap();

    let create_proposed_result = st.create_proposed_transactions::<Infallible, _>(
        account.usk(),
        OvkPolicy::Sender,
        &proposal,
    );
    assert_matches!(
        &create_proposed_result,
        Err(data_api::error::Error::PaysEphemeralTransparentAddress(address_str)) if address_str == &ephemeral0);

    // Simulate another wallet sending to an ephemeral address with an index
    // within the current gap limit. The `PaysEphemeralTransparentAddress` error
    // prevents us from doing so straightforwardly, so we'll do it by building
    // a transaction and calling `store_decrypted_tx` with it.
    let known_addrs = st
        .wallet()
        .get_known_ephemeral_addresses(account_id, None)
        .unwrap();
    assert_eq!(known_addrs.len(), (GAP_LIMIT as usize) + 2);

    // Check that the addresses are all distinct.
    let known_set: HashSet<_> = known_addrs.iter().map(|(addr, _)| addr).collect();
    assert_eq!(known_set.len(), known_addrs.len());
    // Check that the metadata is as expected.
    for (i, (_, meta)) in known_addrs.iter().enumerate() {
        assert_eq!(
            meta,
            &TransparentAddressMetadata::new(
                TransparentKeyScope::EPHEMERAL,
                NonHardenedChildIndex::from_index(i.try_into().unwrap()).unwrap()
            )
        );
    }

    let mut builder = Builder::new(
        *st.network(),
        height + 1,
        BuildConfig::Standard {
            sapling_anchor: None,
            orchard_anchor: None,
        },
    );
    let (colliding_addr, _) = &known_addrs[10];
    let utxo_value = (value - zip317::MINIMUM_FEE).unwrap();
    assert_matches!(
        builder.add_transparent_output(colliding_addr, utxo_value),
        Ok(_)
    );
    let sk = account
        .usk()
        .transparent()
        .derive_secret_key(Scope::External.into(), default_index)
        .unwrap();
    let outpoint = OutPoint::fake();
    let txout = TxOut {
        script_pubkey: default_addr.script(),
        value,
    };
    // Add the fake input to our UTXO set so that we can ensure we recognize the outpoint.
    st.wallet_mut()
        .put_received_transparent_utxo(
            &WalletTransparentOutput::from_parts(outpoint.clone(), txout.clone(), None).unwrap(),
        )
        .unwrap();

    assert_matches!(builder.add_transparent_input(sk, outpoint, txout), Ok(_));
    let test_prover = LocalTxProver::bundled();
    let build_result = builder
        .build(
            OsRng,
            &test_prover,
            &test_prover,
            &zip317::FeeRule::standard(),
        )
        .unwrap();
    let txid = build_result.transaction().txid();
    st.wallet_mut()
        .store_decrypted_tx(DecryptedTransaction::<DSF::AccountId>::new(
            None,
            build_result.transaction(),
            vec![],
            #[cfg(feature = "orchard")]
            vec![],
        ))
        .unwrap();

    // Verify that storing the fully transparent transaction causes a transaction
    // status request to be generated.
    let tx_data_requests = st.wallet().transaction_data_requests().unwrap();
    assert!(tx_data_requests.contains(&TransactionDataRequest::GetStatus(txid)));

    // We call get_wallet_transparent_output with `allow_unspendable = true` to verify
    // storage because the decrypted transaction has not yet been mined.
    let utxo = st
        .wallet()
        .get_all_unspent_transparent_output(&OutPoint::new(txid.into(), 0), true)
        .unwrap();
    assert_matches!(utxo, Some(v) if v.value() == utxo_value);

    // That should have advanced the start of the gap to index 11.
    let new_known_addrs = st
        .wallet()
        .get_known_ephemeral_addresses(account_id, None)
        .unwrap();
    assert_eq!(new_known_addrs.len(), (GAP_LIMIT as usize) + 11);
    assert!(new_known_addrs.starts_with(&known_addrs));

    let reservation_should_succeed = |st: &mut TestState<_, DSF::DataStore, _>, n| {
        let reserved = st
            .wallet_mut()
            .reserve_next_n_ephemeral_addresses(account_id, n)
            .unwrap();
        assert_eq!(reserved.len(), n);
        reserved
    };
    let reservation_should_fail =
        |st: &mut TestState<_, DSF::DataStore, _>, n, _expected_bad_index| {
            assert_matches!(
                st.wallet_mut()
                    .reserve_next_n_ephemeral_addresses(account_id, n),
                Err(..)
            );
            // previously it was hard-coded to a SqlClientError which could check the index
            // not sure how best to handle this in a generic way
            // Err(SqliteClientError::ReachedGapLimit(acct, bad_index))
            // if acct == account_id && bad_index == expected_bad_index);
        };

    let next_reserved = reservation_should_succeed(&mut st, 1);
    assert_eq!(next_reserved[0], known_addrs[11]);

    // Calling `reserve_next_n_ephemeral_addresses(account_id, 1)` will have advanced
    // the start of the gap to index 12. This also tests the `index_range` parameter.
    let newer_known_addrs = st
        .wallet()
        .get_known_ephemeral_addresses(account_id, Some(5..100))
        .unwrap();
    assert_eq!(newer_known_addrs.len(), (GAP_LIMIT as usize) + 12 - 5);
    assert!(newer_known_addrs.starts_with(&new_known_addrs[5..]));

    // None of the five transactions created above (two from each proposal and the
    // one built manually) have been mined yet. So, the range of address indices
    // that are safe to reserve is still 0..20, and we have already reserved 12
    // addresses, so trying to reserve another 9 should fail.
    reservation_should_fail(&mut st, 9, 20);
    reservation_should_succeed(&mut st, 8);
    reservation_should_fail(&mut st, 1, 20);

    // Now mine the transaction with the ephemeral output at index 1.
    // We already reserved 20 addresses, so this should allow 2 more (..22).
    // It does not matter that the transaction with ephemeral output at index 0
    // remains unmined.
    let (h, _) = st.generate_next_block_including(txids1.head);
    st.scan_cached_blocks(h, 1);
    reservation_should_succeed(&mut st, 2);
    reservation_should_fail(&mut st, 1, 22);

    // Mining the transaction with the ephemeral output at index 0 at this point
    // should make no difference.
    let (h, _) = st.generate_next_block_including(txids0.head);
    st.scan_cached_blocks(h, 1);
    reservation_should_fail(&mut st, 1, 22);

    // Now mine the transaction with the ephemeral output at index 10.
    let tx = build_result.transaction();
    let tx_index = 1;
    let (h, _) = st.generate_next_block_from_tx(tx_index, tx);
    st.scan_cached_blocks(h, 1);

    // The above `scan_cached_blocks` does not detect `tx` as interesting to the
    // wallet. If a transaction is in the database with a null `mined_height`,
    // as in this case, its `mined_height` will remain null unless either
    // `put_tx_meta` or `set_transaction_status` is called on it. The former
    // is normally called internally via `put_blocks` as a result of scanning,
    // but not for the case of a fully transparent transaction. The latter is
    // called by the wallet implementation in response to processing the
    // `transaction_data_requests` queue.

    // The reservation should fail because `tx` is not yet seen as mined.
    reservation_should_fail(&mut st, 1, 22);

    // Simulate the wallet processing the `transaction_data_requests` queue.
    let tx_data_requests = st.wallet().transaction_data_requests().unwrap();
    assert!(tx_data_requests.contains(&TransactionDataRequest::GetStatus(tx.txid())));

    // Respond to the GetStatus request.
    st.wallet_mut()
        .set_transaction_status(tx.txid(), TransactionStatus::Mined(h))
        .unwrap();

    // We already reserved 22 addresses, so mining the transaction with the
    // ephemeral output at index 10 should allow 9 more (..31).
    reservation_should_succeed(&mut st, 9);
    reservation_should_fail(&mut st, 1, 31);

    let newest_known_addrs = st
        .wallet()
        .get_known_ephemeral_addresses(account_id, None)
        .unwrap();
    assert_eq!(newest_known_addrs.len(), (GAP_LIMIT as usize) + 31);
    assert!(newest_known_addrs.starts_with(&known_addrs));
    assert!(newest_known_addrs[5..].starts_with(&newer_known_addrs));
}

#[cfg(feature = "transparent-inputs")]
pub fn proposal_fails_if_not_all_ephemeral_outputs_consumed<T: ShieldedPoolTester>(
    dsf: impl DataStoreFactory,
    cache: impl TestCache,
) {
    use crate::proposal::{Proposal, StepOutput, StepOutputIndex};
    use nonempty::NonEmpty;

    let mut st = TestBuilder::new()
        .with_data_store_factory(dsf)
        .with_block_cache(cache)
        .with_account_from_sapling_activation(BlockHash([0; 32]))
        .build();

    let account = st.test_account().cloned().unwrap();
    let account_id = account.id();
    let dfvk = T::test_account_fvk(&st);

    let const_from_u64 = NonNegativeAmount::const_from_u64(100000);
    let value = const_from_u64;
    let transfer_amount = NonNegativeAmount::const_from_u64(50000);

    // Add funds to the wallet.
    st.add_funds(&account, &dfvk, value);

    // Generate a ZIP 320 proposal, sending to the wallet's default transparent address
    // expressed as a TEX address.
    let tex_addr = match account.usk().default_transparent_address().0 {
        TransparentAddress::PublicKeyHash(data) => Address::Tex(data),
        _ => unreachable!(),
    };

    let proposal = st
        .propose_standard_transfer::<Infallible>(
            account_id,
            StandardFeeRule::Zip317,
            NonZeroU32::new(1).unwrap(),
            &tex_addr,
            transfer_amount,
            None,
            None,
            T::SHIELDED_PROTOCOL,
        )
        .unwrap();

    // This is somewhat redundant with `send_multi_step_proposed_transfer`,
    // but tests the case with no change memo and ensures we haven't messed
    // up the test setup.
    let create_proposed_result = st.create_proposed_transactions::<Infallible, _>(
        account.usk(),
        OvkPolicy::Sender,
        &proposal,
    );
    assert_matches!(create_proposed_result, Ok(_));

    // Frobnicate the proposal to make it invalid because it does not consume
    // the ephemeral output, by truncating it to the first step.
    let frobbed_proposal = Proposal::multi_step(
        *proposal.fee_rule(),
        proposal.min_target_height(),
        NonEmpty::singleton(proposal.steps().first().clone()),
    )
    .unwrap();

    let create_proposed_result = st.create_proposed_transactions::<Infallible, _>(
        account.usk(),
        OvkPolicy::Sender,
        &frobbed_proposal,
    );
    assert_matches!(
        create_proposed_result,
        Err(data_api::error::Error::Proposal(crate::proposal::ProposalError::EphemeralOutputLeftUnspent(so)))
        if so == StepOutput::new(0, StepOutputIndex::Change(1))
    );
}

#[allow(deprecated)]
pub fn create_to_address_fails_on_incorrect_usk<T: ShieldedPoolTester>(dsf: impl DataStoreFactory) {
    let mut st = TestBuilder::new()
        .with_data_store_factory(dsf)
        .with_account_from_sapling_activation(BlockHash([0; 32]))
        .build();
    let dfvk = T::test_account_fvk(&st);
    let to = T::fvk_default_address(&dfvk);

    // Create a USK that doesn't exist in the wallet
    let acct1 = zip32::AccountId::try_from(1).unwrap();
    let usk1 = UnifiedSpendingKey::from_seed(st.network(), &[1u8; 32], acct1).unwrap();

    // Attempting to spend with a USK that is not in the wallet results in an error
    assert_matches!(
        st.create_spend_to_address(
            &usk1,
            &to,
            NonNegativeAmount::const_from_u64(1),
            None,
            OvkPolicy::Sender,
            NonZeroU32::new(1).unwrap(),
            None,
            T::SHIELDED_PROTOCOL,
        ),
        Err(data_api::error::Error::KeyNotRecognized)
    );
}

#[allow(deprecated)]
pub fn proposal_fails_with_no_blocks<T: ShieldedPoolTester, D: DataStoreFactory>(dsf: D)
where
    <D as DataStoreFactory>::AccountId: std::fmt::Debug,
{
    let mut st = TestBuilder::new()
        .with_data_store_factory(dsf)
        .with_account_from_sapling_activation(BlockHash([0; 32]))
        .build();

    let account_id = st.test_account().unwrap().id();
    let dfvk = T::test_account_fvk(&st);
    let to = T::fvk_default_address(&dfvk);

    // Wallet summary is not yet available
    assert_eq!(st.get_wallet_summary(0), None);

    // We cannot do anything if we aren't synchronised
    assert_matches!(
        st.propose_standard_transfer::<Infallible>(
            account_id,
            StandardFeeRule::PreZip313,
            NonZeroU32::new(1).unwrap(),
            &to,
            NonNegativeAmount::const_from_u64(1),
            None,
            None,
            T::SHIELDED_PROTOCOL,
        ),
        Err(data_api::error::Error::ScanRequired)
    );
}

pub fn spend_fails_on_unverified_notes<T: ShieldedPoolTester>(
    dsf: impl DataStoreFactory,
    cache: impl TestCache,
) {
    let mut st = TestBuilder::new()
        .with_data_store_factory(dsf)
        .with_block_cache(cache)
        .with_account_from_sapling_activation(BlockHash([0; 32]))
        .build();

    let account = st.test_account().cloned().unwrap();
    let account_id = account.id();
    let dfvk = T::test_account_fvk(&st);

    // Add funds to the wallet in a single note
    let value = NonNegativeAmount::const_from_u64(50000);
    let (h1, _, _) = st.generate_next_block(&dfvk, AddressType::DefaultExternal, value);
    st.scan_cached_blocks(h1, 1);

    // Spendable balance matches total balance at 1 confirmation.
    assert_eq!(st.get_total_balance(account_id), value);
    assert_eq!(st.get_spendable_balance(account_id, 1), value);

    // Value is considered pending at 10 confirmations.
    assert_eq!(st.get_pending_shielded_balance(account_id, 10), value);
    assert_eq!(
        st.get_spendable_balance(account_id, 10),
        NonNegativeAmount::ZERO
    );

    // Wallet is fully scanned
    let summary = st.get_wallet_summary(1);
    assert_eq!(
        summary.and_then(|s| s.scan_progress()),
        Some(Ratio::new(1, 1))
    );

    // Add more funds to the wallet in a second note
    let (h2, _, _) = st.generate_next_block(&dfvk, AddressType::DefaultExternal, value);
    st.scan_cached_blocks(h2, 1);

    // Verified balance does not include the second note
    let total = (value + value).unwrap();
    assert_eq!(st.get_spendable_balance(account_id, 2), value);
    assert_eq!(st.get_pending_shielded_balance(account_id, 2), value);
    assert_eq!(st.get_total_balance(account_id), total);

    // Wallet is still fully scanned
    let summary = st.get_wallet_summary(1);
    assert_eq!(
        summary.and_then(|s| s.scan_progress()),
        Some(Ratio::new(2, 2))
    );

    // Spend fails because there are insufficient verified notes
    let extsk2 = T::sk(&[0xf5; 32]);
    let to = T::sk_default_address(&extsk2);
    assert_matches!(
        st.propose_standard_transfer::<Infallible>(
            account_id,
            StandardFeeRule::Zip317,
            NonZeroU32::new(2).unwrap(),
            &to,
            NonNegativeAmount::const_from_u64(70000),
            None,
            None,
            T::SHIELDED_PROTOCOL,
        ),
        Err(data_api::error::Error::InsufficientFunds {
            available,
            required
        })
        if available == NonNegativeAmount::const_from_u64(50000)
            && required == NonNegativeAmount::const_from_u64(80000)
    );

    // Mine blocks SAPLING_ACTIVATION_HEIGHT + 2 to 9 until just before the second
    // note is verified
    for _ in 2..10 {
        st.generate_next_block(&dfvk, AddressType::DefaultExternal, value);
    }
    st.scan_cached_blocks(h2 + 1, 8);

    // Total balance is value * number of blocks scanned (10).
    assert_eq!(st.get_total_balance(account_id), (value * 10).unwrap());

    // Spend still fails
    assert_matches!(
        st.propose_standard_transfer::<Infallible>(
            account_id,
            StandardFeeRule::Zip317,
            NonZeroU32::new(10).unwrap(),
            &to,
            NonNegativeAmount::const_from_u64(70000),
            None,
            None,
            T::SHIELDED_PROTOCOL,
        ),
        Err(data_api::error::Error::InsufficientFunds {
            available,
            required
        })
        if available == NonNegativeAmount::const_from_u64(50000)
            && required == NonNegativeAmount::const_from_u64(80000)
    );

    // Mine block 11 so that the second note becomes verified
    let (h11, _, _) = st.generate_next_block(&dfvk, AddressType::DefaultExternal, value);
    st.scan_cached_blocks(h11, 1);

    // Total balance is value * number of blocks scanned (11).
    assert_eq!(st.get_total_balance(account_id), (value * 11).unwrap());
    // Spendable balance at 10 confirmations is value * 2.
    assert_eq!(
        st.get_spendable_balance(account_id, 10),
        (value * 2).unwrap()
    );
    assert_eq!(
        st.get_pending_shielded_balance(account_id, 10),
        (value * 9).unwrap()
    );

    // Should now be able to generate a proposal
    let amount_sent = NonNegativeAmount::from_u64(70000).unwrap();
    let min_confirmations = NonZeroU32::new(10).unwrap();
    let proposal = st
        .propose_standard_transfer::<Infallible>(
            account_id,
            StandardFeeRule::Zip317,
            min_confirmations,
            &to,
            amount_sent,
            None,
            None,
            T::SHIELDED_PROTOCOL,
        )
        .unwrap();

    // Executing the proposal should succeed
    let txid = st
        .create_proposed_transactions::<Infallible, _>(account.usk(), OvkPolicy::Sender, &proposal)
        .unwrap()[0];

    let (h, _) = st.generate_next_block_including(txid);
    st.scan_cached_blocks(h, 1);

    // TODO: send to an account so that we can check its balance.
    assert_eq!(
        st.get_total_balance(account_id),
        ((value * 11).unwrap()
            - (amount_sent + NonNegativeAmount::from_u64(10000).unwrap()).unwrap())
        .unwrap()
    );
}

pub fn spend_fails_on_locked_notes<T: ShieldedPoolTester>(
    dsf: impl DataStoreFactory,
    cache: impl TestCache,
) {
    let mut st = TestBuilder::new()
        .with_data_store_factory(dsf)
        .with_block_cache(cache)
        .with_account_from_sapling_activation(BlockHash([0; 32]))
        .build();

    let account = st.test_account().cloned().unwrap();
    let account_id = account.id();
    let dfvk = T::test_account_fvk(&st);

    let fee_rule = StandardFeeRule::Zip317;

    // Add funds to the wallet in a single note
    let value = NonNegativeAmount::const_from_u64(50000);
    let (h1, _, _) = st.generate_next_block(&dfvk, AddressType::DefaultExternal, value);
    st.scan_cached_blocks(h1, 1);

    // Spendable balance matches total balance at 1 confirmation.
    assert_eq!(st.get_total_balance(account_id), value);
    assert_eq!(st.get_spendable_balance(account_id, 1), value);

    // Send some of the funds to another address, but don't mine the tx.
    let extsk2 = T::sk(&[0xf5; 32]);
    let to = T::sk_default_address(&extsk2);
    let min_confirmations = NonZeroU32::new(1).unwrap();
    let proposal = st
        .propose_standard_transfer::<Infallible>(
            account_id,
            fee_rule,
            min_confirmations,
            &to,
            NonNegativeAmount::const_from_u64(15000),
            None,
            None,
            T::SHIELDED_PROTOCOL,
        )
        .unwrap();

    // Executing the proposal should succeed
    assert_matches!(
        st.create_proposed_transactions::<Infallible, _>(account.usk(), OvkPolicy::Sender, &proposal,),
        Ok(txids) if txids.len() == 1
    );

    // A second proposal fails because there are no usable notes
    assert_matches!(
        st.propose_standard_transfer::<Infallible>(
            account_id,
            fee_rule,
            NonZeroU32::new(1).unwrap(),
            &to,
            NonNegativeAmount::const_from_u64(2000),
            None,
            None,
            T::SHIELDED_PROTOCOL,
        ),
        Err(data_api::error::Error::InsufficientFunds {
            available,
            required
        })
        if available == NonNegativeAmount::ZERO && required == NonNegativeAmount::const_from_u64(12000)
    );

    // Mine blocks SAPLING_ACTIVATION_HEIGHT + 1 to 41 (that don't send us funds)
    // until just before the first transaction expires
    for i in 1..42 {
        st.generate_next_block(
            &T::sk_to_fvk(&T::sk(&[i as u8; 32])),
            AddressType::DefaultExternal,
            value,
        );
    }
    st.scan_cached_blocks(h1 + 1, 40);

    // Second proposal still fails
    assert_matches!(
        st.propose_standard_transfer::<Infallible>(
            account_id,
            fee_rule,
            NonZeroU32::new(1).unwrap(),
            &to,
            NonNegativeAmount::const_from_u64(2000),
            None,
            None,
            T::SHIELDED_PROTOCOL,
        ),
        Err(data_api::error::Error::InsufficientFunds {
            available,
            required
        })
        if available == NonNegativeAmount::ZERO && required == NonNegativeAmount::const_from_u64(12000)
    );

    // Mine block SAPLING_ACTIVATION_HEIGHT + 42 so that the first transaction expires
    let (h43, _, _) = st.generate_next_block(
        &T::sk_to_fvk(&T::sk(&[42; 32])),
        AddressType::DefaultExternal,
        value,
    );
    st.scan_cached_blocks(h43, 1);

    // Spendable balance matches total balance at 1 confirmation.
    assert_eq!(st.get_total_balance(account_id), value);
    assert_eq!(st.get_spendable_balance(account_id, 1), value);

    // Second spend should now succeed
    let amount_sent2 = NonNegativeAmount::const_from_u64(2000);
    let min_confirmations = NonZeroU32::new(1).unwrap();
    let proposal = st
        .propose_standard_transfer::<Infallible>(
            account_id,
            fee_rule,
            min_confirmations,
            &to,
            amount_sent2,
            None,
            None,
            T::SHIELDED_PROTOCOL,
        )
        .unwrap();

    let txid2 = st
        .create_proposed_transactions::<Infallible, _>(account.usk(), OvkPolicy::Sender, &proposal)
        .unwrap()[0];

    let (h, _) = st.generate_next_block_including(txid2);
    st.scan_cached_blocks(h, 1);

    // TODO: send to an account so that we can check its balance.
    assert_eq!(
        st.get_total_balance(account_id),
        (value - (amount_sent2 + NonNegativeAmount::from_u64(10000).unwrap()).unwrap()).unwrap()
    );
}

pub fn ovk_policy_prevents_recovery_from_chain<T: ShieldedPoolTester, DSF: DataStoreFactory>(
    dsf: DSF,
    cache: impl TestCache,
) {
    let mut st = TestBuilder::new()
        .with_data_store_factory(dsf)
        .with_block_cache(cache)
        .with_account_from_sapling_activation(BlockHash([0; 32]))
        .build();

    let account = st.test_account().cloned().unwrap();
    let account_id = account.id();
    let dfvk = T::test_account_fvk(&st);

    // Add funds to the wallet in a single note
    let value = NonNegativeAmount::const_from_u64(50000);
    let (h1, _, _) = st.generate_next_block(&dfvk, AddressType::DefaultExternal, value);
    st.scan_cached_blocks(h1, 1);

    // Spendable balance matches total balance at 1 confirmation.
    assert_eq!(st.get_total_balance(account_id), value);
    assert_eq!(st.get_spendable_balance(account_id, 1), value);

    let extsk2 = T::sk(&[0xf5; 32]);
    let addr2 = T::sk_default_address(&extsk2);

    let fee_rule = StandardFeeRule::Zip317;

    #[allow(clippy::type_complexity)]
    let send_and_recover_with_policy = |st: &mut TestState<_, DSF::DataStore, _>,
                                        ovk_policy|
     -> Result<Option<(Note, Address, MemoBytes)>, ()> {
        let min_confirmations = NonZeroU32::new(1).unwrap();
        let proposal = st
            .propose_standard_transfer::<Infallible>(
                account_id,
                fee_rule,
                min_confirmations,
                &addr2,
                NonNegativeAmount::const_from_u64(15000),
                None,
                None,
                T::SHIELDED_PROTOCOL,
            )
            .unwrap();

        // Executing the proposal should succeed
        let txid = st
            .create_proposed_transactions::<Infallible, _>(account.usk(), ovk_policy, &proposal)
            .unwrap()[0];

        // Fetch the transaction from the database
        let tx = st.wallet().get_transaction(txid).unwrap().unwrap();

        Ok(T::try_output_recovery(st.network(), h1, &tx, &dfvk))
    };

    // Send some of the funds to another address, keeping history.
    // The recipient output is decryptable by the sender.
    assert_matches!(
        send_and_recover_with_policy(&mut st, OvkPolicy::Sender),
        Ok(Some((_, recovered_to, _))) if recovered_to == addr2
    );

    // Mine blocks SAPLING_ACTIVATION_HEIGHT + 1 to 42 (that don't send us funds)
    // so that the first transaction expires
    for i in 1..=42 {
        st.generate_next_block(
            &T::sk_to_fvk(&T::sk(&[i as u8; 32])),
            AddressType::DefaultExternal,
            value,
        );
    }
    st.scan_cached_blocks(h1 + 1, 42);

    // Send the funds again, discarding history.
    // Neither transaction output is decryptable by the sender.
    assert_matches!(
        send_and_recover_with_policy(&mut st, OvkPolicy::Discard),
        Ok(None)
    );
}

pub fn spend_succeeds_to_t_addr_zero_change<T: ShieldedPoolTester>(
    dsf: impl DataStoreFactory,
    cache: impl TestCache,
) {
    let mut st = TestBuilder::new()
        .with_data_store_factory(dsf)
        .with_block_cache(cache)
        .with_account_from_sapling_activation(BlockHash([0; 32]))
        .build();

    let account = st.test_account().cloned().unwrap();
    let account_id = account.id();
    let dfvk = T::test_account_fvk(&st);

    // Add funds to the wallet in a single note
    let value = NonNegativeAmount::const_from_u64(70000);
    let (h, _, _) = st.generate_next_block(&dfvk, AddressType::DefaultExternal, value);
    st.scan_cached_blocks(h, 1);

    // Spendable balance matches total balance at 1 confirmation.
    assert_eq!(st.get_total_balance(account_id), value);
    assert_eq!(st.get_spendable_balance(account_id, 1), value);

    let fee_rule = StandardFeeRule::Zip317;

    // TODO: generate_next_block_from_tx does not currently support transparent outputs.
    let to = TransparentAddress::PublicKeyHash([7; 20]).into();
    let min_confirmations = NonZeroU32::new(1).unwrap();
    let proposal = st
        .propose_standard_transfer::<Infallible>(
            account_id,
            fee_rule,
            min_confirmations,
            &to,
            NonNegativeAmount::const_from_u64(50000),
            None,
            None,
            T::SHIELDED_PROTOCOL,
        )
        .unwrap();

    // Executing the proposal should succeed
    assert_matches!(
        st.create_proposed_transactions::<Infallible, _>(account.usk(), OvkPolicy::Sender, &proposal),
        Ok(txids) if txids.len() == 1
    );
}

pub fn change_note_spends_succeed<T: ShieldedPoolTester, DSF: DataStoreFactory>(
    dsf: DSF,
    cache: impl TestCache,
) {
    let mut st = TestBuilder::new()
        .with_data_store_factory(dsf)
        .with_block_cache(cache)
        .with_account_from_sapling_activation(BlockHash([0; 32]))
        .build();

    let account = st.test_account().cloned().unwrap();
    let account_id = account.id();
    let dfvk = T::test_account_fvk(&st);

    // Add funds to the wallet in a single note owned by the internal spending key
    let value = NonNegativeAmount::const_from_u64(70000);
    let (h, _, _) = st.generate_next_block(&dfvk, AddressType::Internal, value);
    st.scan_cached_blocks(h, 1);

    // Spendable balance matches total balance at 1 confirmation.
    assert_eq!(st.get_total_balance(account_id), value);
    assert_eq!(st.get_spendable_balance(account_id, 1), value);

    // Value is considered pending at 10 confirmations.
    assert_eq!(st.get_pending_shielded_balance(account_id, 10), value);
    assert_eq!(
        st.get_spendable_balance(account_id, 10),
        NonNegativeAmount::ZERO
    );

    // NOTE: In prior test this was filtered by the notes value to ensure a match
    //      unfortunately we don't have access to this here but there is only a single note
    //      so it doesn't matter unless the test is modified
    let change_note_scope = st
        .wallet()
        .get_notes(T::SHIELDED_PROTOCOL)
        .unwrap()
        .iter()
        .map(|note: &ReceivedNote<_, _>| note.spending_key_scope())
        .next();

    assert_matches!(change_note_scope, Some(Scope::Internal));

    let fee_rule = StandardFeeRule::Zip317;

    // TODO: generate_next_block_from_tx does not currently support transparent outputs.
    let to = TransparentAddress::PublicKeyHash([7; 20]).into();
    let min_confirmations = NonZeroU32::new(1).unwrap();
    let proposal = st
        .propose_standard_transfer::<Infallible>(
            account_id,
            fee_rule,
            min_confirmations,
            &to,
            NonNegativeAmount::const_from_u64(50000),
            None,
            None,
            T::SHIELDED_PROTOCOL,
        )
        .unwrap();

    // Executing the proposal should succeed
    assert_matches!(
        st.create_proposed_transactions::<Infallible, _>(account.usk(), OvkPolicy::Sender, &proposal),
        Ok(txids) if txids.len() == 1
    );
}

pub fn external_address_change_spends_detected_in_restore_from_seed<T: ShieldedPoolTester, DSF>(
    dsf: DSF,
    cache: impl TestCache,
) where
    DSF: DataStoreFactory,
    <DSF as DataStoreFactory>::DataStore: Reset,
{
    let mut st = TestBuilder::new()
        .with_data_store_factory(dsf)
        .with_block_cache(cache)
        .build();

    // Add two accounts to the wallet.
    let seed = Secret::new([0u8; 32].to_vec());
    let birthday = AccountBirthday::from_sapling_activation(st.network(), BlockHash([0; 32]));
    let (account_id, usk) = st.wallet_mut().create_account(&seed, &birthday).unwrap();
    let dfvk = T::sk_to_fvk(T::usk_to_sk(&usk));

    let (account2, usk2) = st.wallet_mut().create_account(&seed, &birthday).unwrap();
    let dfvk2 = T::sk_to_fvk(T::usk_to_sk(&usk2));

    // Add funds to the wallet in a single note
    let value = NonNegativeAmount::from_u64(100000).unwrap();
    let (h, _, _) = st.generate_next_block(&dfvk, AddressType::DefaultExternal, value);
    st.scan_cached_blocks(h, 1);

    // Spendable balance matches total balance
    assert_eq!(st.get_total_balance(account_id), value);
    assert_eq!(st.get_spendable_balance(account_id, 1), value);
    assert_eq!(st.get_total_balance(account2), NonNegativeAmount::ZERO);

    let amount_sent = NonNegativeAmount::from_u64(20000).unwrap();
    let amount_legacy_change = NonNegativeAmount::from_u64(30000).unwrap();
    let addr = T::fvk_default_address(&dfvk);
    let addr2 = T::fvk_default_address(&dfvk2);
    let req = TransactionRequest::new(vec![
        // payment to an external recipient
        Payment::without_memo(addr2.to_zcash_address(st.network()), amount_sent),
        // payment back to the originating wallet, simulating legacy change
        Payment::without_memo(addr.to_zcash_address(st.network()), amount_legacy_change),
    ])
    .unwrap();

    #[allow(deprecated)]
    let fee_rule = FixedFeeRule::standard();
    let input_selector = GreedyInputSelector::new(
        SingleOutputChangeStrategy::new(fee_rule, None, T::SHIELDED_PROTOCOL),
        DustOutputPolicy::default(),
    );

    let txid = st
        .spend(
            &input_selector,
            &usk,
            req,
            OvkPolicy::Sender,
            NonZeroU32::new(1).unwrap(),
        )
        .unwrap()[0];

    let amount_left = (value - (amount_sent + fee_rule.fixed_fee()).unwrap()).unwrap();
    let pending_change = (amount_left - amount_legacy_change).unwrap();

    // The "legacy change" is not counted by get_pending_change().
    assert_eq!(st.get_pending_change(account_id, 1), pending_change);
    // We spent the only note so we only have pending change.
    assert_eq!(st.get_total_balance(account_id), pending_change);

    let (h, _) = st.generate_next_block_including(txid);
    st.scan_cached_blocks(h, 1);

    assert_eq!(st.get_total_balance(account2), amount_sent,);
    assert_eq!(st.get_total_balance(account_id), amount_left);

    st.reset();

    // Account creation and DFVK derivation should be deterministic.
    let (_, restored_usk) = st.wallet_mut().create_account(&seed, &birthday).unwrap();
    assert!(T::fvks_equal(
        &T::sk_to_fvk(T::usk_to_sk(&restored_usk)),
        &dfvk,
    ));

    let (_, restored_usk2) = st.wallet_mut().create_account(&seed, &birthday).unwrap();
    assert!(T::fvks_equal(
        &T::sk_to_fvk(T::usk_to_sk(&restored_usk2)),
        &dfvk2,
    ));

    st.scan_cached_blocks(st.sapling_activation_height(), 2);

    assert_eq!(st.get_total_balance(account2), amount_sent,);
    assert_eq!(st.get_total_balance(account_id), amount_left);
}

#[allow(dead_code)]
pub fn zip317_spend<T: ShieldedPoolTester>(dsf: impl DataStoreFactory, cache: impl TestCache) {
    let mut st = TestBuilder::new()
        .with_data_store_factory(dsf)
        .with_block_cache(cache)
        .with_account_from_sapling_activation(BlockHash([0; 32]))
        .build();

    let account = st.test_account().cloned().unwrap();
    let account_id = account.id();
    let dfvk = T::test_account_fvk(&st);

    // Add funds to the wallet
    let (h1, _, _) = st.generate_next_block(
        &dfvk,
        AddressType::Internal,
        NonNegativeAmount::const_from_u64(50000),
    );

    // Add 10 dust notes to the wallet
    for _ in 1..=10 {
        st.generate_next_block(
            &dfvk,
            AddressType::DefaultExternal,
            NonNegativeAmount::const_from_u64(1000),
        );
    }

    st.scan_cached_blocks(h1, 11);

    // Spendable balance matches total balance
    let total = NonNegativeAmount::const_from_u64(60000);
    assert_eq!(st.get_total_balance(account_id), total);
    assert_eq!(st.get_spendable_balance(account_id, 1), total);

    let input_selector = input_selector(StandardFeeRule::Zip317, None, T::SHIELDED_PROTOCOL);

    // This first request will fail due to insufficient non-dust funds
    let req = TransactionRequest::new(vec![Payment::without_memo(
        T::fvk_default_address(&dfvk).to_zcash_address(st.network()),
        NonNegativeAmount::const_from_u64(50000),
    )])
    .unwrap();

    assert_matches!(
        st.spend(
            &input_selector,
            account.usk(),
            req,
            OvkPolicy::Sender,
            NonZeroU32::new(1).unwrap(),
        ),
        Err(data_api::error::Error::InsufficientFunds { available, required })
            if available == NonNegativeAmount::const_from_u64(51000)
            && required == NonNegativeAmount::const_from_u64(60000)
    );

    // This request will succeed, spending a single dust input to pay the 10000
    // ZAT fee in addition to the 41000 ZAT output to the recipient
    let req = TransactionRequest::new(vec![Payment::without_memo(
        T::fvk_default_address(&dfvk).to_zcash_address(st.network()),
        NonNegativeAmount::const_from_u64(41000),
    )])
    .unwrap();

    let txid = st
        .spend(
            &input_selector,
            account.usk(),
            req,
            OvkPolicy::Sender,
            NonZeroU32::new(1).unwrap(),
        )
        .unwrap()[0];

    let (h, _) = st.generate_next_block_including(txid);
    st.scan_cached_blocks(h, 1);

    // TODO: send to an account so that we can check its balance.
    // We sent back to the same account so the amount_sent should be included
    // in the total balance.
    assert_eq!(
        st.get_total_balance(account_id),
        (total - NonNegativeAmount::const_from_u64(10000)).unwrap()
    );
}

#[cfg(feature = "transparent-inputs")]
pub fn shield_transparent<T: ShieldedPoolTester, DSF>(dsf: DSF, cache: impl TestCache)
where
    DSF: DataStoreFactory,
    <<DSF as DataStoreFactory>::DataStore as WalletWrite>::UtxoRef: std::fmt::Debug,
{
    use zcash_primitives::transaction::components::{OutPoint, TxOut};

    use crate::wallet::WalletTransparentOutput;

    let mut st = TestBuilder::new()
        .with_data_store_factory(dsf)
        .with_block_cache(cache)
        .with_account_from_sapling_activation(BlockHash([0; 32]))
        .build();

    let account = st.test_account().cloned().unwrap();
    let dfvk = T::test_account_fvk(&st);

    let uaddr = st
        .wallet()
        .get_current_address(account.id())
        .unwrap()
        .unwrap();
    let taddr = uaddr.transparent().unwrap();

    // Ensure that the wallet has at least one block
    let (h, _, _) = st.generate_next_block(
        &dfvk,
        AddressType::Internal,
        NonNegativeAmount::const_from_u64(50000),
    );
    st.scan_cached_blocks(h, 1);

    let utxo = WalletTransparentOutput::from_parts(
        OutPoint::fake(),
        TxOut {
            value: NonNegativeAmount::const_from_u64(100000),
            script_pubkey: taddr.script(),
        },
        Some(h),
    )
    .unwrap();

    let res0 = st.wallet_mut().put_received_transparent_utxo(&utxo);
    assert_matches!(res0, Ok(_));

    let fee_rule = StandardFeeRule::Zip317;

    let input_selector = GreedyInputSelector::new(
        standard::SingleOutputChangeStrategy::new(fee_rule, None, T::SHIELDED_PROTOCOL),
        DustOutputPolicy::default(),
    );

    let txids = st
        .shield_transparent_funds(
            &input_selector,
            NonNegativeAmount::from_u64(10000).unwrap(),
            account.usk(),
            &[*taddr],
            1,
        )
        .unwrap();
    assert_eq!(txids.len(), 1);

    let tx = st.get_tx_from_history(*txids.first()).unwrap().unwrap();
    assert_eq!(tx.spent_note_count(), 1);
    assert!(tx.has_change());
    assert_eq!(tx.received_note_count(), 0);
    assert_eq!(tx.sent_note_count(), 0);
    assert!(tx.is_shielding());
}

// FIXME: This requires fixes to the test framework.
#[allow(dead_code)]
pub fn birthday_in_anchor_shard<T: ShieldedPoolTester>(
    dsf: impl DataStoreFactory,
    cache: impl TestCache,
) {
    // Set up the following situation:
    //
    //        |<------ 500 ------->|<--- 10 --->|<--- 10 --->|
    // last_shard_start   wallet_birthday  received_tx  anchor_height
    //
    // We set the Sapling and Orchard frontiers at the birthday block initial state to 1234
    // notes beyond the end of the first shard.
    let frontier_tree_size: u32 = (0x1 << 16) + 1234;
    let mut st = TestBuilder::new()
        .with_data_store_factory(dsf)
        .with_block_cache(cache)
        .with_initial_chain_state(|rng, network| {
            let birthday_height = network.activation_height(NetworkUpgrade::Nu5).unwrap() + 1000;

            // Construct a fake chain state for the end of the block with the given
            // birthday_offset from the Nu5 birthday.
            let (prior_sapling_roots, sapling_initial_tree) =
                Frontier::random_with_prior_subtree_roots(
                    rng,
                    frontier_tree_size.into(),
                    NonZeroU8::new(16).unwrap(),
                );
            // There will only be one prior root
            let prior_sapling_roots = prior_sapling_roots
                .into_iter()
                .map(|root| CommitmentTreeRoot::from_parts(birthday_height - 500, root))
                .collect::<Vec<_>>();

            #[cfg(feature = "orchard")]
            let (prior_orchard_roots, orchard_initial_tree) =
                Frontier::random_with_prior_subtree_roots(
                    rng,
                    frontier_tree_size.into(),
                    NonZeroU8::new(16).unwrap(),
                );
            // There will only be one prior root
            #[cfg(feature = "orchard")]
            let prior_orchard_roots = prior_orchard_roots
                .into_iter()
                .map(|root| CommitmentTreeRoot::from_parts(birthday_height - 500, root))
                .collect::<Vec<_>>();

            InitialChainState {
                chain_state: ChainState::new(
                    birthday_height - 1,
                    BlockHash([5; 32]),
                    sapling_initial_tree,
                    #[cfg(feature = "orchard")]
                    orchard_initial_tree,
                ),
                prior_sapling_roots,
                #[cfg(feature = "orchard")]
                prior_orchard_roots,
            }
        })
        .with_account_having_current_birthday()
        .build();

    // Generate 9 blocks that have no value for us, starting at the birthday height.
    let not_our_value = NonNegativeAmount::const_from_u64(10000);
    let not_our_key = T::random_fvk(st.rng_mut());
    let (initial_height, _, _) =
        st.generate_next_block(&not_our_key, AddressType::DefaultExternal, not_our_value);
    for _ in 1..9 {
        st.generate_next_block(&not_our_key, AddressType::DefaultExternal, not_our_value);
    }

    // Now, generate a block that belongs to our wallet
    let (received_tx_height, _, _) = st.generate_next_block(
        &T::test_account_fvk(&st),
        AddressType::DefaultExternal,
        NonNegativeAmount::const_from_u64(500000),
    );

    // Generate some more blocks to get above our anchor height
    for _ in 0..15 {
        st.generate_next_block(&not_our_key, AddressType::DefaultExternal, not_our_value);
    }

    // Scan a block range that includes our received note, but skips some blocks we need to
    // make it spendable.
    st.scan_cached_blocks(initial_height + 5, 20);

    // Verify that the received note is not considered spendable
    let account = st.test_account().unwrap();
    let account_id = account.id();
    let spendable = T::select_spendable_notes(
        &st,
        account_id,
        NonNegativeAmount::const_from_u64(300000),
        received_tx_height + 10,
        &[],
    )
    .unwrap();

    assert_eq!(spendable.len(), 0);

    // Scan the blocks we skipped
    st.scan_cached_blocks(initial_height, 5);

    // Verify that the received note is now considered spendable
    let spendable = T::select_spendable_notes(
        &st,
        account_id,
        NonNegativeAmount::const_from_u64(300000),
        received_tx_height + 10,
        &[],
    )
    .unwrap();

    assert_eq!(spendable.len(), 1);
}

pub fn checkpoint_gaps<T: ShieldedPoolTester>(dsf: impl DataStoreFactory, cache: impl TestCache) {
    let mut st = TestBuilder::new()
        .with_data_store_factory(dsf)
        .with_block_cache(cache)
        .with_account_from_sapling_activation(BlockHash([0; 32]))
        .build();

    let account = st.test_account().cloned().unwrap();
    let dfvk = T::test_account_fvk(&st);

    // Generate a block with funds belonging to our wallet.
    st.generate_next_block(
        &dfvk,
        AddressType::DefaultExternal,
        NonNegativeAmount::const_from_u64(500000),
    );
    st.scan_cached_blocks(account.birthday().height(), 1);

    // Create a gap of 10 blocks having no shielded outputs, then add a block that doesn't
    // belong to us so that we can get a checkpoint in the tree.
    let not_our_key = T::sk_to_fvk(&T::sk(&[0xf5; 32]));
    let not_our_value = NonNegativeAmount::const_from_u64(10000);
    st.generate_block_at(
        account.birthday().height() + 10,
        BlockHash([0; 32]),
        &[FakeCompactOutput::new(
            &not_our_key,
            AddressType::DefaultExternal,
            not_our_value,
        )],
        st.latest_cached_block().unwrap().sapling_end_size(),
        st.latest_cached_block().unwrap().orchard_end_size(),
        false,
    );

    // Scan the block
    st.scan_cached_blocks(account.birthday().height() + 10, 1);

    // Fake that everything has been scanned
    // TODO: Add methods for updating scan queue (it seems to work without though..)
    // st.wallet()
    //     .conn()
    //     .execute_batch("UPDATE scan_queue SET priority = 10")
    //     .unwrap();

    // Verify that our note is considered spendable
    let spendable = T::select_spendable_notes(
        &st,
        account.id(),
        NonNegativeAmount::const_from_u64(300000),
        account.birthday().height() + 5,
        &[],
    )
    .unwrap();
    assert_eq!(spendable.len(), 1);

    // Attempt to spend the note with 5 confirmations
    let to = T::fvk_default_address(&not_our_key);
    assert_matches!(
        st.create_spend_to_address(
            account.usk(),
            &to,
            NonNegativeAmount::const_from_u64(10000),
            None,
            OvkPolicy::Sender,
            NonZeroU32::new(5).unwrap(),
            None,
            T::SHIELDED_PROTOCOL,
        ),
        Ok(_)
    );
}

#[cfg(feature = "orchard")]
pub fn pool_crossing_required<P0: ShieldedPoolTester, P1: ShieldedPoolTester>(
    dsf: impl DataStoreFactory,
    cache: impl TestCache,
) {
    let mut st = TestBuilder::new()
        .with_data_store_factory(dsf)
        .with_block_cache(cache)
        .with_account_from_sapling_activation(BlockHash([0; 32])) // TODO: Allow for Orchard
        // activation after Sapling
        .build();

    let account = st.test_account().cloned().unwrap();

    let p0_fvk = P0::test_account_fvk(&st);

    let p1_fvk = P1::test_account_fvk(&st);
    let p1_to = P1::fvk_default_address(&p1_fvk);

    let note_value = NonNegativeAmount::const_from_u64(350000);
    st.generate_next_block(&p0_fvk, AddressType::DefaultExternal, note_value);
    st.scan_cached_blocks(account.birthday().height(), 2);

    let initial_balance = note_value;
    assert_eq!(st.get_total_balance(account.id()), initial_balance);
    assert_eq!(st.get_spendable_balance(account.id(), 1), initial_balance);

    let transfer_amount = NonNegativeAmount::const_from_u64(200000);
    let p0_to_p1 = TransactionRequest::new(vec![Payment::without_memo(
        p1_to.to_zcash_address(st.network()),
        transfer_amount,
    )])
    .unwrap();

    let fee_rule = StandardFeeRule::Zip317;
    let input_selector = GreedyInputSelector::new(
        standard::SingleOutputChangeStrategy::new(fee_rule, None, P1::SHIELDED_PROTOCOL),
        DustOutputPolicy::default(),
    );
    let proposal0 = st
        .propose_transfer(
            account.id(),
            &input_selector,
            p0_to_p1,
            NonZeroU32::new(1).unwrap(),
        )
        .unwrap();

    let _min_target_height = proposal0.min_target_height();
    assert_eq!(proposal0.steps().len(), 1);
    let step0 = &proposal0.steps().head;

    // We expect 4 logical actions, two per pool (due to padding).
    let expected_fee = NonNegativeAmount::const_from_u64(20000);
    assert_eq!(step0.balance().fee_required(), expected_fee);

    let expected_change = (note_value - transfer_amount - expected_fee).unwrap();
    let proposed_change = step0.balance().proposed_change();
    assert_eq!(proposed_change.len(), 1);
    let change_output = proposed_change.get(0).unwrap();
    // Since this is a cross-pool transfer, change will be sent to the preferred pool.
    assert_eq!(
        change_output.output_pool(),
        zcash_protocol::PoolType::Shielded(std::cmp::max(
            ShieldedProtocol::Sapling,
            ShieldedProtocol::Orchard
        ))
    );
    assert_eq!(change_output.value(), expected_change);

    let create_proposed_result = st.create_proposed_transactions::<Infallible, _>(
        account.usk(),
        OvkPolicy::Sender,
        &proposal0,
    );
    assert_matches!(&create_proposed_result, Ok(txids) if txids.len() == 1);

    let (h, _) = st.generate_next_block_including(create_proposed_result.unwrap()[0]);
    st.scan_cached_blocks(h, 1);

    assert_eq!(
        st.get_total_balance(account.id()),
        (initial_balance - expected_fee).unwrap()
    );
    assert_eq!(
        st.get_spendable_balance(account.id(), 1),
        (initial_balance - expected_fee).unwrap()
    );
}

#[cfg(feature = "orchard")]
pub fn fully_funded_fully_private<P0: ShieldedPoolTester, P1: ShieldedPoolTester>(
    dsf: impl DataStoreFactory,
    cache: impl TestCache,
) {
    let mut st = TestBuilder::new()
        .with_data_store_factory(dsf)
        .with_block_cache(cache)
        .with_account_from_sapling_activation(BlockHash([0; 32])) // TODO: Allow for Orchard
        // activation after Sapling
        .build();

    let account = st.test_account().cloned().unwrap();

    let p0_fvk = P0::test_account_fvk(&st);

    let p1_fvk = P1::test_account_fvk(&st);
    let p1_to = P1::fvk_default_address(&p1_fvk);

    let note_value = NonNegativeAmount::const_from_u64(350000);
    st.generate_next_block(&p0_fvk, AddressType::DefaultExternal, note_value);
    st.generate_next_block(&p1_fvk, AddressType::DefaultExternal, note_value);
    st.scan_cached_blocks(account.birthday().height(), 2);

    let initial_balance = (note_value * 2).unwrap();
    assert_eq!(st.get_total_balance(account.id()), initial_balance);
    assert_eq!(st.get_spendable_balance(account.id(), 1), initial_balance);

    let transfer_amount = NonNegativeAmount::const_from_u64(200000);
    let p0_to_p1 = TransactionRequest::new(vec![Payment::without_memo(
        p1_to.to_zcash_address(st.network()),
        transfer_amount,
    )])
    .unwrap();

    let fee_rule = StandardFeeRule::Zip317;
    let input_selector = GreedyInputSelector::new(
        // We set the default change output pool to P0, because we want to verify later that
        // change is actually sent to P1 (as the transaction is fully fundable from P1).
        standard::SingleOutputChangeStrategy::new(fee_rule, None, P0::SHIELDED_PROTOCOL),
        DustOutputPolicy::default(),
    );
    let proposal0 = st
        .propose_transfer(
            account.id(),
            &input_selector,
            p0_to_p1,
            NonZeroU32::new(1).unwrap(),
        )
        .unwrap();

    let _min_target_height = proposal0.min_target_height();
    assert_eq!(proposal0.steps().len(), 1);
    let step0 = &proposal0.steps().head;

    // We expect 2 logical actions, since either pool can pay the full balance required
    // and note selection should choose the fully-private path.
    let expected_fee = NonNegativeAmount::const_from_u64(10000);
    assert_eq!(step0.balance().fee_required(), expected_fee);

    let expected_change = (note_value - transfer_amount - expected_fee).unwrap();
    let proposed_change = step0.balance().proposed_change();
    assert_eq!(proposed_change.len(), 1);
    let change_output = proposed_change.get(0).unwrap();
    // Since there are sufficient funds in either pool, change is kept in the same pool as
    // the source note (the target pool), and does not necessarily follow preference order.
    assert_eq!(
        change_output.output_pool(),
        zcash_protocol::PoolType::Shielded(P1::SHIELDED_PROTOCOL)
    );
    assert_eq!(change_output.value(), expected_change);

    let create_proposed_result = st.create_proposed_transactions::<Infallible, _>(
        account.usk(),
        OvkPolicy::Sender,
        &proposal0,
    );
    assert_matches!(&create_proposed_result, Ok(txids) if txids.len() == 1);

    let (h, _) = st.generate_next_block_including(create_proposed_result.unwrap()[0]);
    st.scan_cached_blocks(h, 1);

    assert_eq!(
        st.get_total_balance(account.id()),
        (initial_balance - expected_fee).unwrap()
    );
    assert_eq!(
        st.get_spendable_balance(account.id(), 1),
        (initial_balance - expected_fee).unwrap()
    );
}

#[cfg(all(feature = "orchard", feature = "transparent-inputs"))]
pub fn fully_funded_send_to_t<P0: ShieldedPoolTester, P1: ShieldedPoolTester>(
    dsf: impl DataStoreFactory,
    cache: impl TestCache,
) {
    let mut st = TestBuilder::new()
        .with_data_store_factory(dsf)
        .with_block_cache(cache)
        .with_account_from_sapling_activation(BlockHash([0; 32])) // TODO: Allow for Orchard
        // activation after Sapling
        .build();

    let account = st.test_account().cloned().unwrap();

    let p0_fvk = P0::test_account_fvk(&st);
    let p1_fvk = P1::test_account_fvk(&st);
    let (p1_to, _) = account.usk().default_transparent_address();

    let note_value = NonNegativeAmount::const_from_u64(350000);
    st.generate_next_block(&p0_fvk, AddressType::DefaultExternal, note_value);
    st.generate_next_block(&p1_fvk, AddressType::DefaultExternal, note_value);
    st.scan_cached_blocks(account.birthday().height(), 2);

    let initial_balance = (note_value * 2).unwrap();
    assert_eq!(st.get_total_balance(account.id()), initial_balance);
    assert_eq!(st.get_spendable_balance(account.id(), 1), initial_balance);

    let transfer_amount = NonNegativeAmount::const_from_u64(200000);
    let p0_to_p1 = TransactionRequest::new(vec![Payment::without_memo(
        Address::Transparent(p1_to).to_zcash_address(st.network()),
        transfer_amount,
    )])
    .unwrap();

    let fee_rule = StandardFeeRule::Zip317;
    let input_selector = GreedyInputSelector::new(
        // We set the default change output pool to P0, because we want to verify later that
        // change is actually sent to P1 (as the transaction is fully fundable from P1).
        standard::SingleOutputChangeStrategy::new(fee_rule, None, P0::SHIELDED_PROTOCOL),
        DustOutputPolicy::default(),
    );
    let proposal0 = st
        .propose_transfer(
            account.id(),
            &input_selector,
            p0_to_p1,
            NonZeroU32::new(1).unwrap(),
        )
        .unwrap();

    let _min_target_height = proposal0.min_target_height();
    assert_eq!(proposal0.steps().len(), 1);
    let step0 = &proposal0.steps().head;

    // We expect 3 logical actions, one for the transparent output and two for the source pool.
    let expected_fee = NonNegativeAmount::const_from_u64(15000);
    assert_eq!(step0.balance().fee_required(), expected_fee);

    let expected_change = (note_value - transfer_amount - expected_fee).unwrap();
    let proposed_change = step0.balance().proposed_change();
    assert_eq!(proposed_change.len(), 1);
    let change_output = proposed_change.get(0).unwrap();
    // Since there are sufficient funds in either pool, change is kept in the same pool as
    // the source note (the target pool), and does not necessarily follow preference order.
    // The source note will always be sapling, as we spend Sapling funds preferentially.
    assert_eq!(change_output.output_pool(), zcash_protocol::PoolType::SAPLING);
    assert_eq!(change_output.value(), expected_change);

    let create_proposed_result = st.create_proposed_transactions::<Infallible, _>(
        account.usk(),
        OvkPolicy::Sender,
        &proposal0,
    );
    assert_matches!(&create_proposed_result, Ok(txids) if txids.len() == 1);

    let (h, _) = st.generate_next_block_including(create_proposed_result.unwrap()[0]);
    st.scan_cached_blocks(h, 1);

    assert_eq!(
        st.get_total_balance(account.id()),
        (initial_balance - transfer_amount - expected_fee).unwrap()
    );
    assert_eq!(
        st.get_spendable_balance(account.id(), 1),
        (initial_balance - transfer_amount - expected_fee).unwrap()
    );
}

#[cfg(feature = "orchard")]
pub fn multi_pool_checkpoint<P0: ShieldedPoolTester, P1: ShieldedPoolTester>(
    dsf: impl DataStoreFactory,
    cache: impl TestCache,
) {
    let mut st = TestBuilder::new()
        .with_data_store_factory(dsf)
        .with_block_cache(cache)
        .with_account_from_sapling_activation(BlockHash([0; 32])) // TODO: Allow for Orchard
        // activation after Sapling
        .build();

    let account = st.test_account().cloned().unwrap();
    let acct_id = account.id();

    let p0_fvk = P0::test_account_fvk(&st);
    let p1_fvk = P1::test_account_fvk(&st);

    // Add some funds to the wallet; we add two notes to allow successive spends. Also,
    // we will generate a note in the P1 pool to ensure that we have some tree state.
    let note_value = NonNegativeAmount::const_from_u64(500000);
    let (start_height, _, _) =
        st.generate_next_block(&p0_fvk, AddressType::DefaultExternal, note_value);
    st.generate_next_block(&p0_fvk, AddressType::DefaultExternal, note_value);
    st.generate_next_block(&p1_fvk, AddressType::DefaultExternal, note_value);
    let scanned = st.scan_cached_blocks(start_height, 3);

    let next_to_scan = scanned.scanned_range().end;

    let initial_balance = (note_value * 3).unwrap();
    assert_eq!(st.get_total_balance(acct_id), initial_balance);
    assert_eq!(st.get_spendable_balance(acct_id, 1), initial_balance);

    // Generate several empty blocks
    for _ in 0..10 {
        st.generate_empty_block();
    }

    // Scan into the middle of the empty range
    let scanned = st.scan_cached_blocks(next_to_scan, 5);
    let next_to_scan = scanned.scanned_range().end;

    // The initial balance should be unchanged.
    assert_eq!(st.get_total_balance(acct_id), initial_balance);
    assert_eq!(st.get_spendable_balance(acct_id, 1), initial_balance);

    // Set up the fee rule and input selector we'll use for all the transfers.
    let fee_rule = StandardFeeRule::Zip317;
    let input_selector = GreedyInputSelector::new(
        standard::SingleOutputChangeStrategy::new(fee_rule, None, P1::SHIELDED_PROTOCOL),
        DustOutputPolicy::default(),
    );

    // First, send funds just to P0
    let transfer_amount = NonNegativeAmount::const_from_u64(200000);
    let p0_transfer = TransactionRequest::new(vec![Payment::without_memo(
        P0::random_address(st.rng_mut()).to_zcash_address(st.network()),
        transfer_amount,
    )])
    .unwrap();
    let res = st
        .spend(
            &input_selector,
            account.usk(),
            p0_transfer,
            OvkPolicy::Sender,
            NonZeroU32::new(1).unwrap(),
        )
        .unwrap();
    st.generate_next_block_including(*res.first());

    let expected_fee = NonNegativeAmount::const_from_u64(10000);
    let expected_change = (note_value - transfer_amount - expected_fee).unwrap();
    assert_eq!(
        st.get_total_balance(acct_id),
        ((note_value * 2).unwrap() + expected_change).unwrap()
    );
    assert_eq!(st.get_pending_change(acct_id, 1), expected_change);

    // In the next block, send funds to both P0 and P1
    let both_transfer = TransactionRequest::new(vec![
        Payment::without_memo(
            P0::random_address(st.rng_mut()).to_zcash_address(st.network()),
            transfer_amount,
        ),
        Payment::without_memo(
            P1::random_address(st.rng_mut()).to_zcash_address(st.network()),
            transfer_amount,
        ),
    ])
    .unwrap();
    let res = st
        .spend(
            &input_selector,
            account.usk(),
            both_transfer,
            OvkPolicy::Sender,
            NonZeroU32::new(1).unwrap(),
        )
        .unwrap();
    st.generate_next_block_including(*res.first());

    // Generate a few more empty blocks
    for _ in 0..5 {
        st.generate_empty_block();
    }

    // Generate another block with funds for us
    let (max_height, _, _) =
        st.generate_next_block(&p0_fvk, AddressType::DefaultExternal, note_value);

    // Scan everything.
    st.scan_cached_blocks(
        next_to_scan,
        usize::try_from(u32::from(max_height) - u32::from(next_to_scan) + 1).unwrap(),
    );

    let expected_final = (initial_balance + note_value
        - (transfer_amount * 3).unwrap()
        - (expected_fee * 3).unwrap())
    .unwrap();
    assert_eq!(st.get_total_balance(acct_id), expected_final);

    use incrementalmerkletree::Position;

    let expected_checkpoints_p0: Vec<(BlockHeight, ShieldedProtocol, Option<Position>)> = [
        (99999, None),
        (100000, Some(0)),
        (100001, Some(1)),
        (100002, Some(1)),
        (100007, Some(1)), // synthetic checkpoint in empty span from scan start
        (100013, Some(3)),
        (100014, Some(5)),
        (100020, Some(6)),
    ]
    .into_iter()
    .map(|(h, pos)| {
        (
            BlockHeight::from(h),
            P0::SHIELDED_PROTOCOL,
            pos.map(Position::from),
        )
    })
    .collect();

    let expected_checkpoints_p1: Vec<(BlockHeight, ShieldedProtocol, Option<Position>)> = [
        (99999, None),
        (100000, None),
        (100001, None),
        (100002, Some(0)),
        (100007, Some(0)), // synthetic checkpoint in empty span from scan start
        (100013, Some(0)),
        (100014, Some(2)),
        (100020, Some(2)),
    ]
    .into_iter()
    .map(|(h, pos)| {
        (
            BlockHeight::from(h),
            P1::SHIELDED_PROTOCOL,
            pos.map(Position::from),
        )
    })
    .collect();

    let actual_checkpoints = st.wallet().get_checkpoint_history().unwrap();

    assert_eq!(
        actual_checkpoints
            .iter()
            .filter(|(_, p, _)| p == &P0::SHIELDED_PROTOCOL)
            .cloned()
            .collect::<Vec<_>>(),
        expected_checkpoints_p0
    );
    assert_eq!(
        actual_checkpoints
            .iter()
            .filter(|(_, p, _)| p == &P1::SHIELDED_PROTOCOL)
            .cloned()
            .collect::<Vec<_>>(),
        expected_checkpoints_p1
    );
}

#[cfg(feature = "orchard")]
pub fn multi_pool_checkpoints_with_pruning<P0: ShieldedPoolTester, P1: ShieldedPoolTester>(
    dsf: impl DataStoreFactory,
    cache: impl TestCache,
) {
    let mut st = TestBuilder::new()
        .with_data_store_factory(dsf)
        .with_block_cache(cache)
        .with_account_from_sapling_activation(BlockHash([0; 32])) // TODO: Allow for Orchard
        // activation after Sapling
        .build();

    let account = st.test_account().cloned().unwrap();

    let p0_fvk = P0::random_fvk(st.rng_mut());
    let p1_fvk = P1::random_fvk(st.rng_mut());

    let note_value = NonNegativeAmount::const_from_u64(10000);
    // Generate 100 P0 blocks, then 100 P1 blocks, then another 100 P0 blocks.
    for _ in 0..10 {
        for _ in 0..10 {
            st.generate_next_block(&p0_fvk, AddressType::DefaultExternal, note_value);
        }
        for _ in 0..10 {
            st.generate_next_block(&p1_fvk, AddressType::DefaultExternal, note_value);
        }
    }
    st.scan_cached_blocks(account.birthday().height(), 200);
    for _ in 0..100 {
        st.generate_next_block(&p0_fvk, AddressType::DefaultExternal, note_value);
        st.generate_next_block(&p1_fvk, AddressType::DefaultExternal, note_value);
    }
    st.scan_cached_blocks(account.birthday().height() + 200, 200);
}

pub fn valid_chain_states<T: ShieldedPoolTester>(
    dsf: impl DataStoreFactory,
    cache: impl TestCache,
) {
    let mut st = TestBuilder::new()
        .with_data_store_factory(dsf)
        .with_block_cache(cache)
        .with_account_from_sapling_activation(BlockHash([0; 32]))
        .build();

    let dfvk = T::test_account_fvk(&st);

    // Empty chain should return None
    assert_matches!(st.wallet().chain_height(), Ok(None));

    // Create a fake CompactBlock sending value to the address
    let (h1, _, _) = st.generate_next_block(
        &dfvk,
        AddressType::DefaultExternal,
        NonNegativeAmount::const_from_u64(5),
    );

    // Scan the cache
    st.scan_cached_blocks(h1, 1);

    // Create a second fake CompactBlock sending more value to the address
    let (h2, _, _) = st.generate_next_block(
        &dfvk,
        AddressType::DefaultExternal,
        NonNegativeAmount::const_from_u64(7),
    );

    // Scanning should detect no inconsistencies
    st.scan_cached_blocks(h2, 1);
}

// FIXME: This requires fixes to the test framework.
#[allow(dead_code)]
pub fn invalid_chain_cache_disconnected<T: ShieldedPoolTester>(
    dsf: impl DataStoreFactory,
    cache: impl TestCache,
) {
    let mut st = TestBuilder::new()
        .with_data_store_factory(dsf)
        .with_block_cache(cache)
        .with_account_from_sapling_activation(BlockHash([0; 32]))
        .build();

    let dfvk = T::test_account_fvk(&st);

    // Create some fake CompactBlocks
    let (h, _, _) = st.generate_next_block(
        &dfvk,
        AddressType::DefaultExternal,
        NonNegativeAmount::const_from_u64(5),
    );
    let (last_contiguous_height, _, _) = st.generate_next_block(
        &dfvk,
        AddressType::DefaultExternal,
        NonNegativeAmount::const_from_u64(7),
    );

    // Scanning the cache should find no inconsistencies
    st.scan_cached_blocks(h, 2);

    // Create more fake CompactBlocks that don't connect to the scanned ones
    let disconnect_height = last_contiguous_height + 1;
    st.generate_block_at(
        disconnect_height,
        BlockHash([1; 32]),
        &[FakeCompactOutput::new(
            &dfvk,
            AddressType::DefaultExternal,
            NonNegativeAmount::const_from_u64(8),
        )],
        2,
        2,
        true,
    );
    st.generate_next_block(
        &dfvk,
        AddressType::DefaultExternal,
        NonNegativeAmount::const_from_u64(3),
    );

    // Data+cache chain should be invalid at the data/cache boundary
    assert_matches!(
        st.try_scan_cached_blocks(
            disconnect_height,
            2
        ),
        Err(data_api::chain::error::Error::Scan(crate::scanning::ScanError::PrevHashMismatch { at_height }))
            if at_height == disconnect_height
    );
}

pub fn data_db_truncation<T: ShieldedPoolTester, DSF>(dsf: DSF, cache: impl TestCache)
where
    DSF: DataStoreFactory,
    <DSF as DataStoreFactory>::AccountId: std::fmt::Debug,
{
    let mut st = TestBuilder::new()
        .with_data_store_factory(dsf)
        .with_block_cache(cache)
        .with_account_from_sapling_activation(BlockHash([0; 32]))
        .build();

    let account = st.test_account().cloned().unwrap();
    let dfvk = T::test_account_fvk(&st);

    // Wallet summary is not yet available
    assert_eq!(st.get_wallet_summary(0), None);

    // Create fake CompactBlocks sending value to the address
    let value = NonNegativeAmount::const_from_u64(5);
    let value2 = NonNegativeAmount::const_from_u64(7);
    let (h, _, _) = st.generate_next_block(&dfvk, AddressType::DefaultExternal, value);
    st.generate_next_block(&dfvk, AddressType::DefaultExternal, value2);

    // Scan the cache
    st.scan_cached_blocks(h, 2);

    // Spendable balance should reflect both received notes
    assert_eq!(
        st.get_spendable_balance(account.id(), 1),
        (value + value2).unwrap()
    );

    // "Rewind" to height of last scanned block (this is a no-op)
    st.wallet_mut().truncate_to_height(h + 1).unwrap();

    // Spendable balance should be unaltered
    assert_eq!(
        st.get_spendable_balance(account.id(), 1),
        (value + value2).unwrap()
    );

    // Rewind so that one block is dropped
    st.wallet_mut().truncate_to_height(h).unwrap();

    // Spendable balance should only contain the first received note;
    // the rest should be pending.
    assert_eq!(st.get_spendable_balance(account.id(), 1), value);
    assert_eq!(st.get_pending_shielded_balance(account.id(), 1), value2);

    // Scan the cache again
    st.scan_cached_blocks(h, 2);

    // Account balance should again reflect both received notes
    assert_eq!(
        st.get_spendable_balance(account.id(), 1),
        (value + value2).unwrap()
    );
}

pub fn scan_cached_blocks_allows_blocks_out_of_order<T: ShieldedPoolTester>(
    dsf: impl DataStoreFactory,
    cache: impl TestCache,
) {
    let mut st = TestBuilder::new()
        .with_data_store_factory(dsf)
        .with_block_cache(cache)
        .with_account_from_sapling_activation(BlockHash([0; 32]))
        .build();

    let account = st.test_account().cloned().unwrap();
    let dfvk = T::test_account_fvk(&st);

    let value = NonNegativeAmount::const_from_u64(50000);
    let (h1, _, _) = st.generate_next_block(&dfvk, AddressType::DefaultExternal, value);
    st.scan_cached_blocks(h1, 1);
    assert_eq!(st.get_total_balance(account.id()), value);

    // Create blocks to reach height + 2
    let (h2, _, _) = st.generate_next_block(&dfvk, AddressType::DefaultExternal, value);
    let (h3, _, _) = st.generate_next_block(&dfvk, AddressType::DefaultExternal, value);

    // Scan the later block first
    st.scan_cached_blocks(h3, 1);

    // Now scan the block of height height + 1
    st.scan_cached_blocks(h2, 1);
    assert_eq!(
        st.get_total_balance(account.id()),
        NonNegativeAmount::const_from_u64(150_000)
    );

    // We can spend the received notes
    let req = TransactionRequest::new(vec![Payment::without_memo(
        T::fvk_default_address(&dfvk).to_zcash_address(st.network()),
        NonNegativeAmount::const_from_u64(110_000),
    )])
    .unwrap();

    #[allow(deprecated)]
    let input_selector = GreedyInputSelector::new(
        standard::SingleOutputChangeStrategy::new(
            StandardFeeRule::Zip317,
            None,
            T::SHIELDED_PROTOCOL,
        ),
        DustOutputPolicy::default(),
    );
    assert_matches!(
        st.spend(
            &input_selector,
            account.usk(),
            req,
            OvkPolicy::Sender,
            NonZeroU32::new(1).unwrap(),
        ),
        Ok(_)
    );
}

pub fn scan_cached_blocks_finds_received_notes<T: ShieldedPoolTester, DSF>(
    dsf: DSF,
    cache: impl TestCache,
) where
    DSF: DataStoreFactory,
    <DSF as DataStoreFactory>::AccountId: std::fmt::Debug,
{
    let mut st = TestBuilder::new()
        .with_data_store_factory(dsf)
        .with_block_cache(cache)
        .with_account_from_sapling_activation(BlockHash([0; 32]))
        .build();

    let account = st.test_account().cloned().unwrap();
    let dfvk = T::test_account_fvk(&st);

    // Wallet summary is not yet available
    assert_eq!(st.get_wallet_summary(0), None);

    // Create a fake CompactBlock sending value to the address
    let value = NonNegativeAmount::const_from_u64(5);
    let (h1, _, _) = st.generate_next_block(&dfvk, AddressType::DefaultExternal, value);

    // Scan the cache
    let summary = st.scan_cached_blocks(h1, 1);
    assert_eq!(summary.scanned_range().start, h1);
    assert_eq!(summary.scanned_range().end, h1 + 1);
    assert_eq!(T::received_note_count(&summary), 1);

    // Account balance should reflect the received note
    assert_eq!(st.get_total_balance(account.id()), value);

    // Create a second fake CompactBlock sending more value to the address
    let value2 = NonNegativeAmount::const_from_u64(7);
    let (h2, _, _) = st.generate_next_block(&dfvk, AddressType::DefaultExternal, value2);

    // Scan the cache again
    let summary = st.scan_cached_blocks(h2, 1);
    assert_eq!(summary.scanned_range().start, h2);
    assert_eq!(summary.scanned_range().end, h2 + 1);
    assert_eq!(T::received_note_count(&summary), 1);

    // Account balance should reflect both received notes
    assert_eq!(
        st.get_total_balance(account.id()),
        (value + value2).unwrap()
    );
}

// TODO: This test can probably be entirely removed, as the following test duplicates it entirely.
pub fn scan_cached_blocks_finds_change_notes<T: ShieldedPoolTester, DSF>(
    dsf: DSF,
    cache: impl TestCache,
) where
    DSF: DataStoreFactory,
    <DSF as DataStoreFactory>::AccountId: std::fmt::Debug,
{
    let mut st = TestBuilder::new()
        .with_data_store_factory(dsf)
        .with_block_cache(cache)
        .with_account_from_sapling_activation(BlockHash([0; 32]))
        .build();

    let account = st.test_account().cloned().unwrap();
    let dfvk = T::test_account_fvk(&st);

    // Wallet summary is not yet available
    assert_eq!(st.get_wallet_summary(0), None);

    // Create a fake CompactBlock sending value to the address
    let value = NonNegativeAmount::const_from_u64(5);
    let (received_height, _, nf) =
        st.generate_next_block(&dfvk, AddressType::DefaultExternal, value);

    // Scan the cache
    st.scan_cached_blocks(received_height, 1);

    // Account balance should reflect the received note
    assert_eq!(st.get_total_balance(account.id()), value);

    // Create a second fake CompactBlock spending value from the address
    let not_our_key = T::sk_to_fvk(&T::sk(&[0xf5; 32]));
    let to2 = T::fvk_default_address(&not_our_key);
    let value2 = NonNegativeAmount::const_from_u64(2);
    let (spent_height, _) = st.generate_next_block_spending(&dfvk, (nf, value), to2, value2);

    // Scan the cache again
    st.scan_cached_blocks(spent_height, 1);

    // Account balance should equal the change
    assert_eq!(
        st.get_total_balance(account.id()),
        (value - value2).unwrap()
    );
}

pub fn scan_cached_blocks_detects_spends_out_of_order<T: ShieldedPoolTester, DSF>(
    dsf: DSF,
    cache: impl TestCache,
) where
    DSF: DataStoreFactory,
    <DSF as DataStoreFactory>::AccountId: std::fmt::Debug,
{
    let mut st = TestBuilder::new()
        .with_data_store_factory(dsf)
        .with_block_cache(cache)
        .with_account_from_sapling_activation(BlockHash([0; 32]))
        .build();

    let account = st.test_account().cloned().unwrap();
    let dfvk = T::test_account_fvk(&st);

    // Wallet summary is not yet available
    assert_eq!(st.get_wallet_summary(0), None);

    // Create a fake CompactBlock sending value to the address
    let value = NonNegativeAmount::const_from_u64(5);
    let (received_height, _, nf) =
        st.generate_next_block(&dfvk, AddressType::DefaultExternal, value);

    // Create a second fake CompactBlock spending value from the address
    let not_our_key = T::sk_to_fvk(&T::sk(&[0xf5; 32]));
    let to2 = T::fvk_default_address(&not_our_key);
    let value2 = NonNegativeAmount::const_from_u64(2);
    let (spent_height, _) = st.generate_next_block_spending(&dfvk, (nf, value), to2, value2);

    // Scan the spending block first.
    st.scan_cached_blocks(spent_height, 1);

    // Account balance should equal the change
    assert_eq!(
        st.get_total_balance(account.id()),
        (value - value2).unwrap()
    );

    // Now scan the block in which we received the note that was spent.
    st.scan_cached_blocks(received_height, 1);

    // Account balance should be the same.
    assert_eq!(
        st.get_total_balance(account.id()),
        (value - value2).unwrap()
    );
}