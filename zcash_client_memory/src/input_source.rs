use zcash_client_backend::data_api::{InputSource, TransactionStatus, WalletRead};
use zcash_client_backend::wallet::{Note, ReceivedNote};
use zcash_primitives::transaction::components::OutPoint;
use zcash_protocol::{
    consensus,
    consensus::BlockHeight,
    value::Zatoshis,
    ShieldedProtocol::{Orchard, Sapling},
};
#[cfg(feature = "transparent-inputs")]
use {
    zcash_client_backend::wallet::WalletTransparentOutput,
    zcash_primitives::legacy::TransparentAddress,
};

use crate::transparent::{ReceivedTransparentOutput, TransparentReceivedOutputs};
use crate::{error::Error, to_spendable_notes, AccountId, MemoryWalletDb, NoteId};

impl<P: consensus::Parameters> InputSource for MemoryWalletDb<P> {
    type Error = crate::error::Error;
    type AccountId = AccountId;
    type NoteRef = NoteId;

    /// Find the note with the given index (output index for Sapling, action index for Orchard)
    /// that belongs to the given transaction
    fn get_spendable_note(
        &self,
        txid: &zcash_primitives::transaction::TxId,
        protocol: zcash_protocol::ShieldedProtocol,
        index: u32,
    ) -> Result<
        Option<
            zcash_client_backend::wallet::ReceivedNote<
                Self::NoteRef,
                zcash_client_backend::wallet::Note,
            >,
        >,
        Self::Error,
    > {
        let note = self.received_notes.iter().find(|rn| {
            &rn.txid == txid && rn.note.protocol() == protocol && rn.output_index == index
        });

        Ok(if let Some(note) = note {
            if self.note_is_spent(note, 0)? {
                None
            } else {
                Some(zcash_client_backend::wallet::ReceivedNote::from_parts(
                    note.note_id,
                    *txid,
                    index.try_into().unwrap(), // this overflow can never happen or else the chain is broken
                    note.note.clone(),
                    note.recipient_key_scope
                        .ok_or(Error::Missing("recipient key scope".into()))?,
                    note.commitment_tree_position
                        .ok_or(Error::Missing("commitment tree position".into()))?,
                ))
            }
        } else {
            None
        })
    }

    fn select_spendable_notes(
        &self,
        account: Self::AccountId,
        target_value: zcash_protocol::value::Zatoshis,
        sources: &[zcash_protocol::ShieldedProtocol],
        anchor_height: zcash_protocol::consensus::BlockHeight,
        exclude: &[Self::NoteRef],
    ) -> Result<zcash_client_backend::data_api::SpendableNotes<Self::NoteRef>, Self::Error> {
        let sapling_eligible_notes = if sources.contains(&Sapling) {
            self.select_spendable_notes_from_pool(
                account,
                target_value,
                &Sapling,
                anchor_height,
                exclude,
            )?
        } else {
            Vec::new()
        };

        #[cfg(feature = "orchard")]
        let orchard_eligible_notes = if sources.contains(&Orchard) {
            self.select_spendable_notes_from_pool(
                account,
                target_value,
                &Orchard,
                anchor_height,
                exclude,
            )?
        } else {
            Vec::new()
        };

        to_spendable_notes(
            &sapling_eligible_notes,
            #[cfg(feature = "orchard")]
            &orchard_eligible_notes,
        )
    }

    /// Returns the list of spendable transparent outputs received by this wallet at `address`
    /// such that, at height `target_height`:
    /// * the transaction that produced the output had or will have at least `min_confirmations`
    ///   confirmations; and
    /// * the output is unspent as of the current chain tip.
    ///
    /// An output that is potentially spent by an unmined transaction in the mempool is excluded
    /// iff the spending transaction will not be expired at `target_height`.
    #[cfg(feature = "transparent-inputs")]
    fn get_spendable_transparent_outputs(
        &self,
        address: &TransparentAddress,
        target_height: BlockHeight,
        min_confirmations: u32,
    ) -> Result<Vec<WalletTransparentOutput>, Self::Error> {
        let txos = self
            .transparent_received_outputs
            .iter()
            .filter(|(_, txo)| txo.address == *address)
            .map(|(outpoint, txo)| (outpoint, txo, self.tx_table.get(&txo.transaction_id)))
            .filter(|(outpoint, _, _)| {
                self.utxo_is_spendable(outpoint, target_height, min_confirmations)
                    .unwrap()
            })
            .filter_map(|(outpoint, txo, tx)| {
                txo.to_wallet_transparent_output(outpoint, tx.map(|tx| tx.mined_height()).flatten())
            })
            .collect();
        Ok(txos)
    }

    /// Fetches the transparent output corresponding to the provided `outpoint`.
    ///
    /// Returns `Ok(None)` if the UTXO is not known to belong to the wallet or is not
    /// spendable as of the chain tip height.
    #[cfg(feature = "transparent-inputs")]
    fn get_unspent_transparent_output(
        &self,
        outpoint: &OutPoint,
    ) -> Result<Option<WalletTransparentOutput>, Self::Error> {
        Ok(self
            .transparent_received_outputs
            .get(outpoint)
            .map(|txo| (txo, self.tx_table.get(&txo.transaction_id)))
            .map(|(txo, tx)| {
                txo.to_wallet_transparent_output(outpoint, tx.map(|tx| tx.mined_height()).flatten())
            })
            .flatten())
    }
}

impl<P: consensus::Parameters> MemoryWalletDb<P> {
    // Select the spendable notes to cover the given target value considering only a single pool
    // Returns the notes sorted oldest to newest
    fn select_spendable_notes_from_pool(
        &self,
        account: AccountId,
        target_value: Zatoshis,
        pool: &zcash_protocol::ShieldedProtocol,
        anchor_height: consensus::BlockHeight,
        exclude: &[NoteId],
    ) -> Result<Vec<&crate::ReceivedNote>, Error> {
        let birthday_height = match self.get_wallet_birthday()? {
            Some(birthday) => birthday,
            None => {
                // the wallet birthday can only be unknown if there are no accounts in the wallet; in
                // such a case, the wallet has no notes to spend.
                return Ok(Vec::new());
            }
        };
        // First grab all eligible (unspent, spendable, fully scanned) notes into a vec.
        let mut eligible_notes = self
            .received_notes
            .iter()
            .filter(|note| note.account_id == account)
            .filter(|note| note.note.protocol() == *pool)
            .filter(|note| {
                self.note_is_spendable(note, birthday_height, anchor_height, exclude)
                    .unwrap()
            })
            .collect::<Vec<_>>();

        // sort by oldest first (use location in commitment tree since this gives a total order)
        eligible_notes.sort_by(|a, b| a.commitment_tree_position.cmp(&b.commitment_tree_position));

        // now take notes until we have enough to cover the target value
        let mut value_acc = Zatoshis::ZERO;
        let selection: Vec<_> = eligible_notes
            .into_iter()
            .take_while(|note| {
                let take = value_acc <= target_value;
                value_acc = (value_acc + note.note.value()).expect("value overflow");
                take
            })
            .collect();

        Ok(selection)
    }

    pub fn utxo_is_spendable(
        &self,
        outpoint: &OutPoint,
        target_height: BlockHeight,
        min_confirmations: u32,
    ) -> Result<bool, Error> {
        let confirmed_height = target_height - min_confirmations;
        let utxo = self
            .transparent_received_outputs
            .get(outpoint)
            .ok_or(Error::NoteNotFound)?;
        if let Some(tx) = self.tx_table.get(&utxo.transaction_id) {
            Ok(
                tx.is_mined_or_unexpired_at(confirmed_height) // tx that created it is mined
                && !self.utxo_is_spent(outpoint, min_confirmations)?, // not spent
            )
        } else {
            Ok(false)
        }
    }

    fn utxo_is_spent(&self, outpoint: &OutPoint, min_confirmations: u32) -> Result<bool, Error> {
        let spend = self.transparent_received_output_spends.get(outpoint);

        let spent = match spend {
            Some(txid) => {
                let spending_tx = self
                    .tx_table
                    .get(txid)
                    .ok_or_else(|| Error::TransactionNotFound(*txid))?;
                match spending_tx.status() {
                    TransactionStatus::Mined(_height) => true,
                    TransactionStatus::TxidNotRecognized => unreachable!(),
                    TransactionStatus::NotInMainChain => {
                        // check the expiry
                        spending_tx.expiry_height().is_none() // no expiry, tx could be mined any time so we consider it spent
                            // expiry is in the future so it could still be mined
                            || spending_tx.expiry_height() > self.summary_height(min_confirmations)?
                    }
                }
            }
            None => false,
        };
        Ok(spent)
    }
}
