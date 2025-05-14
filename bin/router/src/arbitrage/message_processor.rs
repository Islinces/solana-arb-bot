use crate::interface::{AccountUpdate, GrpcMessage, ReadyGrpcMessageOperator};
use async_channel::{Receiver, Sender};
use moka::ops::compute::{CompResult, Op};
use moka::sync::Cache;
use solana_sdk::pubkey::Pubkey;
use std::time::Duration;
use tracing::{error, info, warn};

pub struct GrpcDataProcessor {
    events: Option<Cache<(String, Pubkey), GrpcMessage>>,
    source_message_receiver: Receiver<AccountUpdate>,
    ready_update_data_sender: Sender<GrpcMessage>,
}

impl GrpcDataProcessor {
    pub fn new(
        need_cache: bool,
        event_capacity: u64,
        event_expired_mills: u64,
        source_message_receiver: Receiver<AccountUpdate>,
        cache_update_sender: Sender<GrpcMessage>,
    ) -> Self {
        Self {
            events: if need_cache {
                Some(
                    Cache::builder()
                        .max_capacity(event_capacity)
                        .time_to_live(Duration::from_millis(event_expired_mills))
                        .build(),
                )
            } else {
                None
            },
            source_message_receiver,
            ready_update_data_sender: cache_update_sender,
        }
    }

    fn get_ready_data(
        &self,
        update_account: AccountUpdate,
        operator: Box<dyn ReadyGrpcMessageOperator>,
    ) -> Option<GrpcMessage> {
        match operator.parse_message(update_account) {
            Ok((cache_key, grpc_message)) => {
                // 不需要等待其他账户数据的情况
                if cache_key.is_none() {
                    return Some(grpc_message);
                }
                let entry = self
                    .events
                    .as_ref()
                    .unwrap()
                    .entry(cache_key.unwrap())
                    .and_upsert_with(|maybe_entry| {
                        if let Some(entry) = maybe_entry {
                            let mut message = entry.into_value();
                            let _ = operator.change_data(&mut message, grpc_message);
                            message
                        } else {
                            grpc_message
                        }
                    });
                if entry.is_old_value_replaced() {
                    if entry.value().is_ready() {
                        Some(entry.into_value())
                    } else {
                        None
                    }
                } else {
                    None
                }
            }
            Err(_) => None,
        }
    }

    #[tokio::main]
    pub async fn run(self) -> anyhow::Result<()> {
        loop {
            tokio::select! {
                source_message = self.source_message_receiver.recv() => {
                    let update = source_message?;
                    let protocol = update.protocol.clone();
                    if let Ok(operator)=protocol.get_grpc_message_operator(){
                        if let Some(grpc_message)= self.get_ready_data(update,operator){
                            if let Err(e)= self.ready_update_data_sender.send(grpc_message).await{
                                error!("触发Route：发送消息失败，原因：{}",e);
                            }
                        }
                    }else{
                        warn!("更新缓存失败：未找到【{:?}】的【ReadyGrpcMessageOperator】实现",protocol)
                    }
                }else => warn!("source_message_receiver closed")
            }
        }
    }
}
