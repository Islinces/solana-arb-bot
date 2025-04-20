use crate::state::FetchConfig;
use crate::trigger::{TriggerEvent, TriggerEventHolder};
use solana_client::nonblocking::rpc_client::RpcClient;
use solana_program::pubkey::Pubkey;
use std::any::Any;
use std::fmt::Debug;
use std::sync::Arc;
use tokio::sync::mpsc::UnboundedSender;

#[async_trait::async_trait]
pub trait DexInterface: Sync + Send {
    fn name(&self) -> String;
    fn get_base_pools(&self) -> Vec<Arc<dyn DexPoolInterface>>;
    async fn fetch_pool_base_info(
        rpc_client: &RpcClient,
        fetch_config: &FetchConfig,
    ) -> anyhow::Result<Arc<dyn DexInterface>>
    where
        Self: Sized;
}

pub trait DexPoolInterface: Sync + Send + Debug {
    fn get_pool_id(&self) -> Pubkey;

    fn get_mint_0(&self) -> Pubkey;

    fn get_mint_1(&self) -> Pubkey;

    fn get_mint_0_vault(&self) -> Option<Pubkey>;
    fn get_mint_1_vault(&self) -> Option<Pubkey>;
    fn as_any(&self) -> &dyn Any;

    fn quote(&self, amount_in: u64, amount_in_mint: Pubkey) -> Option<u64>;

    fn update_data(&mut self, changed_pool: Box<dyn TriggerEvent>) -> anyhow::Result<Pubkey>;
}

#[async_trait::async_trait]
pub trait GrpcSubscriber: Sync + Send {
    async fn subscribe(
        dex: Arc<dyn DexInterface>,
        fetch_config: Arc<FetchConfig>,
        account_write_sender: UnboundedSender<Box<dyn DexPoolInterface>>,
        trigger_event_sender: UnboundedSender<Box<dyn TriggerEvent>>,
    );
}

pub struct SubscribeRequest {}
