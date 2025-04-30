use async_trait::async_trait;
use burberry::Executor;
use solana_program::pubkey::Pubkey;
use tracing::info;

pub struct GrpcMessageExecutor {}

impl GrpcMessageExecutor {
    pub fn new() -> Self {
        Self {}
    }
}

#[async_trait]
impl Executor<Pubkey> for GrpcMessageExecutor {
    fn name(&self) -> &str {
        "SwapExecutor"
    }

    async fn execute(&self, pool_id: Pubkey) -> eyre::Result<()> {
        info!("触发交易：{pool_id}");
        Ok(())
    }
}
