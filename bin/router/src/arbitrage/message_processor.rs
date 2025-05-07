use crate::interface::{AccountUpdate, GrpcMessage, ReadyGrpcMessageOperator};
use async_channel::{Receiver, Sender};
use moka::ops::compute::{CompResult, Op};
use moka::sync::Cache;
use solana_program::pubkey::Pubkey;
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
        mut operator: Box<dyn ReadyGrpcMessageOperator>,
    ) -> Option<GrpcMessage> {
        if operator.parse_message().is_err() {
            return None;
        }
        // 不需要等待其他账户数据的情况
        if self.events.is_none() {
            return Some(operator.get_insert_data());
        }
        let cache_key = operator.get_cache_key();
        let entry = self
            .events
            .as_ref()
            .unwrap()
            .entry(cache_key)
            .and_compute_with(|maybe_entry| {
                if let Some(exists) = maybe_entry {
                    let mut message = exists.into_value();
                    if operator.change_and_return_ready_data(&mut message).is_ok() {
                        Op::Remove
                    } else {
                        Op::Put(message)
                    }
                } else {
                    Op::Put(operator.get_insert_data())
                }
            });
        match entry {
            CompResult::Removed(r) => Some(r.into_value()),
            _ => None,
        }
    }

    #[tokio::main]
    pub async fn run(self) -> anyhow::Result<()> {
        loop {
            tokio::select! {
                source_message = self.source_message_receiver.recv() => {
                    let update = source_message?;
                    let protocol = update.protocol.clone();
                    if let Ok(operator)=protocol.get_grpc_message_operator(update){
                        if let Some(grpc_message)= self.get_ready_data(operator){
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
