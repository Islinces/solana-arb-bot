use crate::arb_bot::Command;
use crate::quoter::QuoteResult;
use solana_sdk::pubkey;
use solana_sdk::pubkey::Pubkey;
use std::sync::Arc;

pub mod jito;

const MEMO_PROGRAM: Pubkey = pubkey!("Memo1UhkJRfHyvLMcVucJwxXeuD728EqVDDwQDxFMNo");

#[async_trait::async_trait]
pub trait Executor: Sync + Send {
    fn initialize(command: &Command) -> anyhow::Result<Arc<dyn Executor>>
    where
        Self: Sized;

    async fn execute(
        &self,
        quote_result: QuoteResult,
        tx: String,
        slot: u64,
    ) -> anyhow::Result<String>;
}
