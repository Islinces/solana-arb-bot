use crate::defi::dex::Dex;
use crate::defi::raydium_amm::raydium_amm::RaydiumAmmDex;
use crate::defi::raydium_clmm::raydium_clmm::RaydiumClmmDex;
use crate::defi::types::{AccountUpdate, GrpcAccountUpdateType, Pool, Protocol, SourceMessage};
use crate::defi::Defi;
use burberry::{async_trait, Collector, CollectorStream};
use solana_program::pubkey::Pubkey;
use std::collections::HashMap;
use std::time::Duration;
use tracing::{debug, error, info};
use yellowstone_grpc_client::GeyserGrpcClient;
use yellowstone_grpc_proto::geyser::subscribe_update::UpdateOneof;
use yellowstone_grpc_proto::geyser::{
    CommitmentLevel, SubscribeRequest, SubscribeRequestAccountsDataSlice,
    SubscribeRequestFilterAccounts, SubscribeUpdate,
};
use yellowstone_grpc_proto::tonic::codegen::tokio_stream::{Stream, StreamExt, StreamMap};
use yellowstone_grpc_proto::tonic::Status;

pub struct GrpcMessageCollector {
    rpc_url: String,
    subscribe_mints: Vec<Pubkey>,
    grpc_url: &'static str,
    ping_interval_with_secs: u64,
}

impl GrpcMessageCollector {
    pub fn new(
        rpc_url: String,
        subscribe_mints: Vec<Pubkey>,
        grpc_url: &'static str,
        ping_interval_with_secs: u64,
    ) -> Self {
        Self {
            rpc_url,
            subscribe_mints,
            grpc_url,
            ping_interval_with_secs,
        }
    }

    async fn connect_grpc_server(
        &self,
    ) -> anyhow::Result<
        StreamMap<
            (Protocol, GrpcAccountUpdateType),
            impl Stream<Item = Result<SubscribeUpdate, Status>>,
        >,
    > {
        let defi = Defi::new(&self.rpc_url, &self.subscribe_mints)
            .await
            .unwrap();
        let pools = defi.get_all_pools().unwrap();
        let mut grpc_client = GeyserGrpcClient::build_from_static(self.grpc_url)
            .tcp_nodelay(true)
            .http2_adaptive_window(true)
            .buffer_size(65536)
            .initial_connection_window_size(5242880)
            .initial_stream_window_size(4194304)
            .connect_timeout(Duration::from_millis(10 * 1000))
            .connect()
            .await
            .map_err(|e| {
                error!("failed to connect: {e}");
                anyhow::anyhow!(e)
            })
            .unwrap();
        let mut subscrbeitions = StreamMap::new();
        for (protocol, pools) in pools.into_iter() {
            let subscribe_requests = Self::create_subscribe_request(&protocol, &pools);
            for (account_type, subscribe_request) in subscribe_requests {
                let (_, stream) = grpc_client
                    .subscribe_with_request(Some(subscribe_request))
                    .await?;
                subscrbeitions.insert((protocol.clone(), account_type), stream);
            }
            info!("GRPC订阅成功：{:?}", protocol);
        }
        let ping_interval = self.ping_interval_with_secs;
        tokio::spawn(async move {
            let mut ping = tokio::time::interval(Duration::from_secs(ping_interval));
            ping.set_missed_tick_behavior(tokio::time::MissedTickBehavior::Delay);
            ping.tick().await;
            loop {
                tokio::select! {
                    _ = ping.tick() => {
                        if let Err(e)=grpc_client.ping(1).await{
                            error!("GRPC PING 失败，{}",e);
                        }
                    },
                }
            }
        });
        Ok(subscrbeitions)
    }

    fn create_subscribe_request(
        protocol: &Protocol,
        pools: &Vec<Pool>,
    ) -> Vec<(GrpcAccountUpdateType, SubscribeRequest)> {
        match protocol {
            Protocol::RaydiumAMM => RaydiumAmmDex::get_subscribe_request(pools),
            Protocol::RaydiumCLmm => RaydiumClmmDex::get_subscribe_request(pools),
            Protocol::PumpFunAMM => {
                vec![]
            }
            Protocol::MeteoraDLMM => {
                vec![]
            }
        }
    }
}

#[async_trait]
impl Collector<AccountUpdate> for GrpcMessageCollector {
    async fn get_event_stream(&self) -> eyre::Result<CollectorStream<'_, AccountUpdate>> {
        let mut subscrbeitions = self.connect_grpc_server().await.unwrap();
        let stream = async_stream::stream! {
            loop {
                tokio::select! {
                    Some(((protocol,account_type),Ok(data))) = subscrbeitions.next() => {
                        if data.filters.is_empty() {
                            continue;
                        }
                        if let Some(UpdateOneof::Account(account)) = data.update_oneof {
                            info!("GRPC推送消息: {:?}",protocol);
                            yield AccountUpdate{
                                protocol,
                                account_type,
                                filters:data.filters,
                                account,
                            };
                        }
                    },
                }
            }
        };
        Ok(Box::pin(stream))
    }
}
