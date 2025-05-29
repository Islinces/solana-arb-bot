use crate::quoter::QuoteResult;
use std::sync::Arc;
use crate::arb_bot::Command;

pub mod jito;

#[async_trait::async_trait]
pub trait Executor: Sync + Send {
    fn initialize(command: &Command) -> anyhow::Result<Arc<dyn Executor>>
    where
        Self: Sized;

    async fn execute(&self, quote_result: QuoteResult) -> anyhow::Result<String>;
}
