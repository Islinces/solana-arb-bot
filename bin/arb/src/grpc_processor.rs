use crate::account_cache::get_account_data;
use crate::data_slice;
use crate::data_slice::SliceType;
use crate::dex::byte_utils::read_u64;
use crate::dex::pump_fun::state::Pool;
use crate::dex::raydium_amm::state::AmmInfo;
use crate::dex::raydium_clmm::state::{PoolState, TickArrayBitmapExtension, TickArrayState};
use crate::dex::FromCache;
use crate::interface::{AccountType, DexType};
use crate::state::{GrpcMessage, GrpcTransactionMsg};
use ahash::RandomState;
use anyhow::anyhow;
use borsh::BorshDeserialize;
use dashmap::DashMap;
use flume::Receiver;
use futures_util::future::err;
use solana_sdk::pubkey::Pubkey;
use spl_token::solana_program::program_pack::Pack;
use spl_token::state::Account;
use std::ptr;
use std::time::Duration;
use tokio::sync::broadcast;
use tokio::task::JoinSet;
use tokio::time::Instant;
use tracing::{error, info};

pub struct MessageProcessor {
    pub process_size: usize,
}

impl MessageProcessor {
    pub fn new(process_size: usize) -> Self {
        Self { process_size }
    }

    pub async fn start(
        &mut self,
        join_set: &mut JoinSet<()>,
        grpc_message_receiver: &Receiver<GrpcMessage>,
        cached_message_sender: &broadcast::Sender<(GrpcTransactionMsg, Duration, Instant)>,
    ) {
        for _index in 0..self.process_size {
            let cached_message_sender = cached_message_sender.clone();
            let grpc_message_receiver = grpc_message_receiver.clone();
            join_set.spawn(async move {
                loop {
                    match grpc_message_receiver.recv_async().await {
                        Ok(grpc_message) => match grpc_message {
                            GrpcMessage::Account(account_msg) => {
                                let _ = Self::update_cache(
                                    account_msg.owner_key,
                                    account_msg.account_key,
                                    account_msg.data,
                                );
                            }
                            GrpcMessage::Transaction(transaction_msg) => {
                                let grpc_to_processor_cost = transaction_msg.instant.elapsed();
                                match cached_message_sender.send((
                                    transaction_msg,
                                    grpc_to_processor_cost,
                                    Instant::now(),
                                )) {
                                    Ok(_) => {}
                                    Err(e) => {
                                        error!("发送Transaction到Arb失败，原因：{}", e);
                                    }
                                }
                            }
                        },
                        Err(_) => {}
                    };
                }
            });
        }
    }

    fn update_cache(owner: Vec<u8>, account_key: Vec<u8>, mut data: Vec<u8>) -> anyhow::Result<()> {
        let account_key = Pubkey::try_from(account_key)
            .map_or(Err(anyhow!("转换account_key失败")), |a| Ok(a))?;
        let sliced_data = data_slice::slice_data_auto_get_dex_type(
            &account_key,
            &Pubkey::try_from(owner).map_or(Err(anyhow!("转换owner失败")), |a| Ok(a))?,
            data.as_slice(),
            SliceType::Subscribed,
        );
        match sliced_data {
            Ok(sliced_data) => crate::account_cache::update_cache(account_key, sliced_data),
            Err(e) => Err(anyhow!("账户数据切片失败，原因：{}", e)),
        };
        Ok(())
    }
}

// pub fn test(owner: &Pubkey, account: &Pubkey, data: &[u8], t: usize) {
//     match crate::account_relation::get_dex_type_and_account_type(owner, account, data) {
//         None => {}
//         Some((dex_type, account_type, pool_id)) => match dex_type {
//             DexType::RaydiumAMM => match account_type {
//                 AccountType::Pool => match get_account_data::<AmmInfo>(&pool_id) {
//                     None => {}
//                     Some(amm_info) => {
//                         // info!("{t} RaydiumAMM pool {:?}\n{:#?}", pool_id, amm_info)
//                     }
//                 },
//                 AccountType::MintVault => {
//                     // // 现在的
//                     // if t == 2 {
//                     //     match get_account_data::<AmmInfo>(&pool_id) {
//                     //         None => {}
//                     //         Some(amm_info) => {
//                     //             info!("Now RaydiumAMM pool {:?}\n{:#?}", pool_id, amm_info)
//                     //         }
//                     //     }
//                     // } else {
//                     //     unsafe {
//                     //         // 推送的
//                     //         if t == 1 {
//                     //             info!(
//                     //                 "GRPC： RaydiumAMM mint {:?}\n{:#?}",
//                     //                 account,
//                     //                 read_u64(&data[64..64 + 8])
//                     //             );
//                     //         }
//                     //         // 之前的
//                     //         else if t == 3 {
//                     //             info!(
//                     //                 "previous ：  RaydiumAMM mint {:?}\n{:#?}",
//                     //                 account,
//                     //                 read_u64(&data[0..8])
//                     //             );
//                     //         }
//                     //     }
//                     // }
//                 }
//                 _ => {}
//             },
//             DexType::RaydiumCLMM => match account_type {
//                 AccountType::Pool =>
//                 // 现在的
//                 {
//                     // if t == 2 {
//                     //     match get_account_data::<PoolState>(&pool_id) {
//                     //         None => {}
//                     //         Some(amm_info) => {
//                     //             info!("Now RaydiumCLMM pool {:?}\n{:#?}", pool_id, amm_info)
//                     //         }
//                     //     }
//                     // } else {
//                     //     unsafe {
//                     //         // 推送的
//                     //         if t == 1 {
//                     //             info!(
//                     //                 "GRPC： RaydiumCLMM pool {:?}\n{:#?}",
//                     //                 account,
//                     //                 crate::dex::raydium_clmm::copy_pool::PoolState::try_from_slice(
//                     //                     &data[8..]
//                     //                 )
//                     //             );
//                     //         }
//                     //         // 之前的
//                     //         else if t == 3 {
//                     //             info!(
//                     //                 "previous ：  RaydiumCLMM pool {:?}\n{:#?}",
//                     //                 account,
//                     //                 PoolState::from_slice_data(&[], data)
//                     //             );
//                     //         }
//                     //     }
//                     // }
//                 }
//                 AccountType::TickArrayState => {
//                     // // 现在的
//                     // if t == 2 {
//                     //     match get_account_data::<TickArrayState>(account) {
//                     //         None => {}
//                     //         Some(amm_info) => {
//                     //             info!("Now RaydiumCLMM {:?}\n{:#?}", account, amm_info)
//                     //         }
//                     //     }
//                     // } else {
//                     //     unsafe {
//                     //         // 推送的
//                     //         if t == 1 {
//                     //             info!(
//                     //                 "GRPC： RaydiumCLMM {:?}\n{:#?}",
//                     //                 account,
//                     //                 crate::dex::raydium_clmm::copy_tick_array::TickArrayState::try_from_slice(&data[8..])
//                     //             );
//                     //         }
//                     //         // 之前的
//                     //         else if t == 3 {
//                     //             info!(
//                     //                 "previous ： RaydiumCLMM  {:?}\n{:#?}",
//                     //                 account,
//                     //                 unsafe {
//                     //                     Some(ptr::read_unaligned(
//                     //                         data.as_ptr() as *const TickArrayState
//                     //                     ))
//                     //                 }
//                     //             );
//                     //         }
//                     //     }
//                     // }
//                 }
//                 AccountType::TickArrayBitmapExtension => {
//                     // 现在的
//                     // if t == 2 {
//                     //     match get_account_data::<TickArrayBitmapExtension>(account) {
//                     //         None => {}
//                     //         Some(amm_info) => {
//                     //             info!("Now RaydiumCLMM {:?}\n{:#?}", pool_id, amm_info)
//                     //         }
//                     //     }
//                     // } else {
//                     //     unsafe {
//                     //         // 推送的
//                     //         if t == 1 {
//                     //             info!(
//                     //                 "GRPC： RaydiumCLMM {:?}\n{:#?}",
//                     //                 account,
//                     //                 crate::dex::raydium_clmm::copy_tickarray_bitmap_extension::TickArrayBitmapExtension::try_from_slice(&data[8..])
//                     //             );
//                     //         }
//                     //         // 之前的
//                     //         else if t == 3 {
//                     //             info!(
//                     //                 "previous ：  RaydiumCLMM pool {:?}\n{:#?}",
//                     //                 account,
//                     //                 TickArrayBitmapExtension::from_slice_data(data)
//                     //             );
//                     //         }
//                     //     }
//                     // }
//                 }
//                 _ => {}
//             },
//             DexType::PumpFunAMM => match account_type {
//                 AccountType::Pool => match get_account_data::<AmmInfo>(&pool_id) {
//                     None => {}
//                     Some(amm_info) => {
//                         // info!("{t} RaydiumAMM pool {:?}\n{:#?}", pool_id, amm_info)
//                     }
//                 },
//                 AccountType::MintVault => {
//                     // 现在的
//                     if t == 2 {
//                         match get_account_data::<Pool>(&pool_id) {
//                             None => {}
//                             Some(amm_info) => {
//                                 info!("Now PumpFunAMM pool {:?}\n{:#?}", pool_id, amm_info)
//                             }
//                         }
//                     } else {
//                         unsafe {
//                             // 推送的
//                             if t == 1 {
//                                 info!(
//                                     "GRPC： PumpFunAMM mint {:?}\n{:#?}",
//                                     account,
//                                     read_u64(&data[64..64 + 8])
//                                 );
//                             }
//                             // 之前的
//                             else if t == 3 {
//                                 info!(
//                                     "previous ：  PumpFunAMM mint {:?}\n{:#?}",
//                                     account,
//                                     read_u64(&data[0..8])
//                                 );
//                             }
//                         }
//                     }
//                 }
//                 _ => {}
//             },
//             DexType::MeteoraDLMM => {}
//         },
//     }
// }
