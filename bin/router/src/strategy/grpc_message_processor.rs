use crate::defi::raydium_amm::raydium_amm::RaydiumAmmDex;
use crate::defi::types::{
    AccountUpdate, GrpcAccountUpdateType, PoolExtra, Protocol, SourceMessage,
};
use anyhow::anyhow;
use arrayref::{array_ref, array_refs};
use async_channel::{Receiver, Sender};
use base58::ToBase58;
use chrono::Utc;
use moka::ops::compute::{CompResult, Op};
use moka::sync::Cache;
use solana_program::pubkey::Pubkey;
use std::sync::Arc;
use std::time::Duration;
use tracing::info;
use yellowstone_grpc_proto::geyser::SubscribeUpdateAccount;
use GrpcMessage::RaydiumAmmData;

pub struct GrpcDataProcessor {
    events: Arc<Cache<(String, Pubkey), GrpcMessage>>,
    source_message_receiver: Receiver<AccountUpdate>,
    ready_update_data_sender: Sender<GrpcMessage>,
}

impl GrpcDataProcessor {
    pub fn new(
        event_capacity: u64,
        event_expired_mills: u64,
        source_message_receiver: Receiver<AccountUpdate>,
        cache_update_sender: Sender<GrpcMessage>,
    ) -> Self {
        Self {
            events: Arc::new(
                Cache::builder()
                    .max_capacity(event_capacity)
                    .time_to_live(Duration::from_millis(event_expired_mills))
                    .build(),
            ),
            source_message_receiver,
            ready_update_data_sender: cache_update_sender,
        }
    }

    #[tokio::main]
    pub async fn run(mut self) -> anyhow::Result<()> {
        loop {
            tokio::select! {
                source_message = self.source_message_receiver.recv() => {
                    let  update = source_message.unwrap();
                    match update.protocol {
                        Protocol::RaydiumAMM=>{
                            if let Some(update_data)= RaydiumAmmDex::try_get_ready_message(update,self.events.clone()).await{
                                info!("发送更新缓存消息");
                                self.ready_update_data_sender.send(update_data).await?;
                            }
                        },
                        _=>{
                        }
                    }
                }
            }
        }
    }

    // async fn raydium_amm_get_ready_message(
    //     &mut self,
    //     account_type: GrpcAccountUpdateType,
    //     filters: Vec<String>,
    //     account: SubscribeUpdateAccount,
    // ) -> Option<GrpcMessage> {
    //     if let Some(update_account_info) = account.account {
    //         let txn = update_account_info.txn_signature.unwrap().to_base58();
    //         let data = update_account_info.data;
    //         let push_event = match account_type {
    //             GrpcAccountUpdateType::PoolState => {
    //                 let src = array_ref![data, 0, 80];
    //                 let (need_take_pnl_coin, need_take_pnl_pc, _coin_vault_mint, _pc_vault_mint) =
    //                     array_refs![src, 8, 8, 32, 32];
    //                 let pool_id = Pubkey::try_from(update_account_info.pubkey.as_slice()).unwrap();
    //                 Some((
    //                     pool_id,
    //                     RaydiumAmmData {
    //                         pool_id,
    //                         mint_0_vault_amount: None,
    //                         mint_1_vault_amount: None,
    //                         mint_0_need_take_pnl: Some(u64::from_le_bytes(*need_take_pnl_coin)),
    //                         mint_1_need_take_pnl: Some(u64::from_le_bytes(*need_take_pnl_pc)),
    //                     },
    //                 ))
    //             }
    //             GrpcAccountUpdateType::MintVault => {
    //                 let src = array_ref![data, 0, 41];
    //                 let (_mint, amount, _state) = array_refs![src, 32, 8, 1];
    //                 let mut mint_0_vault_amount = None;
    //                 let mut mint_1_vault_amount = None;
    //                 let items = filters.get(0).unwrap().split(":").collect::<Vec<&str>>();
    //                 let mint_flag = items.last().unwrap().to_string();
    //                 if mint_flag.eq("0") {
    //                     mint_0_vault_amount = Some(u64::from_le_bytes(*amount));
    //                 } else {
    //                     mint_1_vault_amount = Some(u64::from_le_bytes(*amount));
    //                 }
    //                 let pool_id = Pubkey::try_from(items.first().unwrap().clone()).unwrap();
    //                 Some((
    //                     pool_id,
    //                     RaydiumAmmData {
    //                         pool_id,
    //                         mint_0_vault_amount,
    //                         mint_1_vault_amount,
    //                         mint_0_need_take_pnl: None,
    //                         mint_1_need_take_pnl: None,
    //                     },
    //                 ))
    //             }
    //             GrpcAccountUpdateType::NONE => None,
    //         };
    //         if let Some((pool_id, update_data)) = push_event {
    //             let entry = self
    //                 .events
    //                 .entry((txn, pool_id))
    //                 .and_compute_with(|maybe_entry| {
    //                     if let Some(exists) = maybe_entry {
    //                         let mut message = exists.into_value();
    //                         if message.fill_change_data(update_data).is_ok() {
    //                             Op::Remove
    //                         } else {
    //                             Op::Put(message)
    //                         }
    //                     } else {
    //                         Op::Put(update_data)
    //                     }
    //                 });
    //             match entry {
    //                 CompResult::Removed(r) => Some(r.into_value()),
    //                 _ => None,
    //             }
    //         } else {
    //             None
    //         }
    //     } else {
    //         None
    //     }
    // }
}

#[derive(Debug, Clone)]
pub enum GrpcMessage {
    RaydiumAmmData {
        pool_id: Pubkey,
        mint_0_vault_amount: Option<u64>,
        mint_1_vault_amount: Option<u64>,
        mint_0_need_take_pnl: Option<u64>,
        mint_1_need_take_pnl: Option<u64>,
    },
    RaydiumClmmData {
        pool_id: Pubkey,
        tick_current: Option<i32>,
        liquidity: Option<u128>,
        sqrt_price_x64: Option<u128>,
        tick_array_bitmap: Option<[u64; 16]>,
    },
}

// impl GrpcMessage {
//     fn fill_change_data(&mut self, update_data: GrpcMessage) -> anyhow::Result<()> {
//         match self {
//             RaydiumAmmData {
//                 mint_0_vault_amount,
//                 mint_1_vault_amount,
//                 mint_0_need_take_pnl,
//                 mint_1_need_take_pnl,
//                 ..
//             } => {
//                 if let RaydiumAmmData {
//                     mint_0_vault_amount: update_mint_0_vault_amount,
//                     mint_1_vault_amount: update_mint_1_vault_amount,
//                     mint_0_need_take_pnl: update_mint_0_need_take_pnl,
//                     mint_1_need_take_pnl: update_mint_1_need_take_pnl,
//                     ..
//                 } = update_data
//                 {
//                     if update_mint_0_vault_amount.is_some() {
//                         *mint_0_vault_amount = update_mint_0_vault_amount;
//                     }
//                     if update_mint_1_vault_amount.is_some() {
//                         *mint_1_vault_amount = update_mint_1_vault_amount;
//                     }
//                     if update_mint_0_need_take_pnl.is_some() {
//                         *mint_0_need_take_pnl = update_mint_0_need_take_pnl;
//                     }
//                     if update_mint_1_need_take_pnl.is_some() {
//                         *mint_1_need_take_pnl = update_mint_1_need_take_pnl;
//                     }
//                     if mint_0_vault_amount.is_some()
//                         && mint_1_vault_amount.is_some()
//                         && mint_0_need_take_pnl.is_some()
//                         && mint_1_need_take_pnl.is_some()
//                     {
//                         Ok(())
//                     } else {
//                         Err(anyhow!(""))
//                     }
//                 } else {
//                     Err(anyhow!(""))
//                 }
//             },
//             _=>{Ok(())}
//         }
//     }
// }
