use crate::dex::DexData;
use crate::interface::{AccountUpdate, SubscribeKey};
use burberry::{async_trait, Collector, CollectorStream};
use std::borrow::Cow;
use std::sync::Arc;
use std::time::{Duration, Instant};
use solana_rpc_client::nonblocking::rpc_client::RpcClient;
use tracing::{error, info, warn};
use yellowstone_grpc_client::{ClientTlsConfig, GeyserGrpcClient};
use yellowstone_grpc_proto::geyser::subscribe_update::UpdateOneof;
use yellowstone_grpc_proto::geyser::SubscribeUpdate;
use yellowstone_grpc_proto::tonic;
use yellowstone_grpc_proto::tonic::codegen::tokio_stream::{Stream, StreamExt, StreamMap};
use yellowstone_grpc_proto::tonic::Status;

pub struct GrpcMessageCollector {
    rpc_client: Arc<RpcClient>,
    grpc_url: String,
    dex_json_path: String,
}

impl GrpcMessageCollector {
    pub fn new(rpc_client: Arc<RpcClient>, grpc_url: String, dex_json_path: String) -> Self {
        Self {
            rpc_client,
            grpc_url,
            dex_json_path,
        }
    }

    async fn connect_grpc_server(
        &self,
    ) -> anyhow::Result<StreamMap<SubscribeKey, impl Stream<Item = Result<SubscribeUpdate, Status>>>>
    {
        let dex =
            DexData::new_only_cache_holder(self.rpc_client.clone(), self.dex_json_path.clone())
                .await?;
        let mut grpc_client = GeyserGrpcClient::build_from_shared(self.grpc_url.clone())?
            .tcp_nodelay(true)
            .http2_adaptive_window(true)
            .buffer_size(65536)
            .initial_connection_window_size(5242880)
            .initial_stream_window_size(4194304)
            .connect_timeout(Duration::from_millis(30 * 1000))
            .tls_config(ClientTlsConfig::new().with_native_roots())?
            .connect()
            .await
            .map_err(|e| {
                error!("GRPC订阅: 连接GRPC服务器失败，原因: {e}");
                anyhow::anyhow!(e)
            })?;
        let mut subscrbeitions = StreamMap::new();
        for (protocol, pools) in dex.get_all_pools().unwrap().into_iter() {
            let subscribe_requests = protocol
                .get_subscribe_request_generator()
                .map(|generator| generator.create_subscribe_requests(&pools));
            if let Ok(Some(subscribe_requests)) = subscribe_requests {
                for (key, subscribe_request) in subscribe_requests {
                    // TODO: 失败重试
                    let (_, stream) = grpc_client
                        .subscribe_with_request(Some(subscribe_request))
                        .await?;
                    subscrbeitions.insert(key, stream);
                }
            } else {
                error!(
                    "GRPC订阅: 【{:?}】未找到GrpcSubscribeRequestGenerator实现或未生成订阅请求",
                    protocol
                );
            }
        }
        if subscrbeitions.is_empty() {
            Err(anyhow::anyhow!("GRPC订阅: 无订阅请求生成，订阅失败"))
        } else {
            tokio::spawn(async move {
                let mut ping = tokio::time::interval(Duration::from_secs(5));
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
            info!(
                "GRPC订阅: 订阅成功列表【{:#?}】",
                subscrbeitions
                    .keys()
                    .map(|s| format!("{:?}", s))
                    .collect::<Vec<_>>()
            );
            Ok(subscrbeitions)
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
                        // PING
                        if data.filters.is_empty() {
                            continue;
                        }
                        if let Some(UpdateOneof::Account(account)) = data.update_oneof {
                            yield AccountUpdate{
                                protocol,
                                account_type,
                                filters:data.filters,
                                account,
                                instant:Instant::now()
                            };
                        }
                    }else => warn!("subscrbeitions closed"),
                }
            }
        };
        Ok(Box::pin(stream))
    }
}
