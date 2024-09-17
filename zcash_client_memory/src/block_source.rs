use std::collections::BTreeMap;
use zcash_client_backend::data_api::chain::BlockSource;
use zcash_client_backend::proto::compact_formats::CompactBlock;

/// A block cache that just holds blocks in a map in memory
pub struct MemBlockCache(pub(crate) BTreeMap<u64, CompactBlock>);

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
