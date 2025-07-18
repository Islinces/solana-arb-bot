use crate::arb_bot::Command;
use crate::HopPathSearchResult;
use std::sync::Arc;

mod jito;

pub use jito::*;

#[async_trait::async_trait]
pub trait Executor: Sync + Send {
    async fn initialize(command: &Command) -> anyhow::Result<Arc<dyn Executor>>
    where
        Self: Sized;

    async fn execute(
        &self,
        hop_path_search_result: HopPathSearchResult,
        tx: String,
        slot: u64,
    ) -> anyhow::Result<String>;
}
