use crate::arb::Arb;
use crate::dex::meteora_damm_v2::state::pool::Pool;
use crate::dex::oracle::Oracle;
use crate::dex::tick_array::TickArray;
use crate::dex::whirlpool::Whirlpool;
use crate::dex::{
    get_account_data, get_dex_type_and_account_type, get_subscribed_accounts, is_follow_vault,
    raydium_cpmm, read_from, update_cache, AccountType, AmmInfo, BinArray, BinArrayBitmapExtension,
    LbPair, MintVault, PoolState, TickArrayBitmapExtension, TickArrayState, CLOCK_ID,
};
use crate::dex::{slice_data_auto_get_dex_type, SliceType};
use crate::dex::{DexType, FromCache};
use crate::grpc_subscribe::{GrpcMessage, GrpcTransactionMsg};
use ahash::{AHashMap, AHashSet, RandomState};
use anyhow::anyhow;
use base58::ToBase58;
use borsh::BorshDeserialize;
use dashmap::DashMap;
use flume::{Receiver, RecvError, TrySendError};
use futures_util::future::err;
use serde::Serialize;
use serde_json::{Map, Value};
use serde_json_diff::Difference;
use solana_sdk::pubkey;
use solana_sdk::pubkey::Pubkey;
use spl_token::solana_program::program_pack::Pack;
use spl_token::state::Account;
use std::collections::HashSet;
use std::fmt::{Debug, Formatter};
use std::ops::{BitAnd, Sub};
use std::ptr;
use std::str::FromStr;
use std::sync::atomic::{AtomicUsize, Ordering};
use std::time::Duration;
use tokio::sync::broadcast;
use tokio::task::JoinSet;
use tokio::time::Instant;
use tracing::{error, info, warn};
use yellowstone_grpc_proto::prelude::{
    Message, TokenBalance, TransactionStatusMeta, UiTokenAmount,
};

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
        cached_message_sender: flume::Sender<GrpcTransactionMsg>,
        cached_message_receiver: Receiver<GrpcTransactionMsg>,
    ) {
        for index in 0..self.process_size {
            let cached_message_sender = cached_message_sender.clone();
            let cached_msg_drop_receiver = cached_message_receiver.clone();
            let grpc_message_receiver = grpc_message_receiver.clone();
            join_set.spawn(async move {
                loop {
                    match grpc_message_receiver.recv_async().await {
                        Ok(grpc_message) => {
                            match grpc_message {
                                GrpcMessage::Account(account_msg) => {
                                    match Self::update_cache(
                                        account_msg.owner_key,
                                        account_msg.account_key,
                                        account_msg.data,
                                    ) {
                                        Ok(_) => {
                                        }
                                        Err(e) => {
                                            error!("更新缓存失败，{}", e);
                                        }
                                    }
                                }
                                GrpcMessage::Transaction(transaction_msg) => {
                                    match cached_message_sender.try_send(transaction_msg) {
                                        Err(TrySendError::Full(msg)) => {
                                            cached_msg_drop_receiver.try_recv().ok();
                                            let mut retry_count = 3;
                                            loop {
                                                if retry_count != 0 {
                                                    match cached_message_sender.try_send(msg.clone()) {
                                                        Err(TrySendError::Full(_)) => {
                                                            cached_msg_drop_receiver.try_recv().ok();
                                                            retry_count -= 1;
                                                        }
                                                        _ => {
                                                            break;
                                                        }
                                                    }
                                                }
                                            }
                                        }
                                        Err(TrySendError::Disconnected(_)) => {
                                            error!("Processor_{index} 发送消息到Arb失败，原因：所有Arb关闭");
                                            break;
                                        }
                                        Ok(_) => {
                                        }
                                    }
                                }
                            }
                        }
                        Err(RecvError::Disconnected) => {
                            error!("Processor_{index} 接收消息失败，原因：Grpc订阅线程关闭");
                            break;
                        }
                    };
                }
            });
        }
    }

    fn update_cache(owner: Vec<u8>, account_key: Vec<u8>, data: Vec<u8>) -> anyhow::Result<()> {
        let account_key = Pubkey::try_from(account_key)
            .map_or(Err(anyhow!("转换account_key失败")), |a| Ok(a))?;
        let owner = Pubkey::try_from(owner).map_or(Err(anyhow!("转换owner失败")), |a| Ok(a))?;
        update_cache(
            account_key,
            slice_data_auto_get_dex_type(&account_key, &owner, data, SliceType::Subscribed)?,
        )?;
        // match get_dex_type_and_account_type(&owner, &account_key) {
        //     None => {}
        //     Some((dex_type, account_type)) => match dex_type {
        //         DexType::RaydiumAMM => match account_type {
        //             AccountType::Pool => {
        //                 info!(
        //                     "account_key : {account_key} , {:#?}",
        //                     get_account_data::<AmmInfo>(&account_key)
        //                 );
        //             }
        //             AccountType::MintVault => {
        //                 info!(
        //                     "account_key : {account_key} , {:#?}",
        //                     get_account_data::<MintVault>(&account_key)
        //                 );
        //             }
        //             _ => {}
        //         },
        //         DexType::RaydiumCLMM => match account_type {
        //             AccountType::Pool => {
        //                 info!(
        //                     "account_key : {account_key} , {:#?}",
        //                     get_account_data::<PoolState>(&account_key)
        //                 );
        //             }
        //             AccountType::TickArray => {
        //                 info!(
        //                     "account_key : {account_key} , {:#?}",
        //                     get_account_data::<TickArrayState>(&account_key)
        //                 );
        //             }
        //             AccountType::TickArrayBitmap => {
        //                 info!(
        //                     "account_key : {account_key} , {:#?}",
        //                     get_account_data::<TickArrayBitmapExtension>(&account_key)
        //                 );
        //             }
        //             _ => {}
        //         },
        //         DexType::RaydiumCPMM => match account_type {
        //             AccountType::Pool => {
        //                 info!(
        //                     "account_key : {account_key} , {:#?}",
        //                     get_account_data::<raydium_cpmm::states::PoolState>(&account_key)
        //                 );
        //             }
        //             AccountType::MintVault => {
        //                 info!(
        //                     "account_key : {account_key} , {:#?}",
        //                     get_account_data::<MintVault>(&account_key)
        //                 );
        //             }
        //             _ => {}
        //         },
        //         DexType::PumpFunAMM => match account_type {
        //             AccountType::MintVault => {
        //                 info!(
        //                     "account_key : {account_key} , {:#?}",
        //                     get_account_data::<MintVault>(&account_key)
        //                 );
        //             }
        //             _ => {}
        //         },
        //         DexType::MeteoraDLMM => match account_type {
        //             AccountType::Pool => {
        //                 info!(
        //                     "account_key : {account_key} , {:#?}",
        //                     get_account_data::<LbPair>(&account_key)
        //                 );
        //             }
        //             AccountType::BinArray => {
        //                 info!(
        //                     "account_key : {account_key} , {:#?}",
        //                     get_account_data::<BinArray>(&account_key)
        //                 );
        //             }
        //             AccountType::BinArrayBitmap => {
        //                 info!(
        //                     "account_key : {account_key} , {:#?}",
        //                     get_account_data::<BinArrayBitmapExtension>(&account_key)
        //                 );
        //             }
        //             _ => {}
        //         },
        //         DexType::MeteoraDAMMV2 => match account_type {
        //             AccountType::Pool => {
        //                 info!(
        //                     "account_key : {account_key} , {:#?}",
        //                     get_account_data::<crate::dex::meteora_damm_v2::state::pool::Pool>(
        //                         &account_key
        //                     )
        //                 );
        //             }
        //             _ => {}
        //         },
        //         DexType::OrcaWhirl => match account_type {
        //             AccountType::Pool => {
        //                 info!(
        //                     "account_key : {account_key} , {:#?}",
        //                     get_account_data::<Whirlpool>(&account_key)
        //                 );
        //             }
        //             AccountType::TickArray => {
        //                 info!(
        //                     "account_key : {account_key} , {:#?}",
        //                     get_account_data::<crate::dex::tick_array::TickArray>(&account_key)
        //                 );
        //             }
        //             AccountType::Oracle => {
        //                 info!(
        //                     "account_key : {account_key} , {:#?}",
        //                     get_account_data::<Oracle>(&account_key)
        //                 );
        //             }
        //             _ => {}
        //         },
        //     },
        // }

        Ok(())
    }
}

pub struct BalanceChangeInfo {
    pub dex_type: DexType,
    pub pool_id: Pubkey,
    pub account_index: usize,
    pub vault_account: Pubkey,
    pub post_account: String,
    pub change_value: f64,
}

impl BalanceChangeInfo {
    fn new(
        tx: &[u8],
        pre: &TokenBalance,
        post: &TokenBalance,
        account_keys: &[Pubkey],
    ) -> Option<Self> {
        let account_index = pre.account_index as usize;
        match (pre.ui_token_amount.as_ref(), post.ui_token_amount.as_ref()) {
            (Some(pre_amount), Some(post_amount)) => {
                if pre_amount.ui_amount == post_amount.ui_amount {
                    None
                } else {
                    let vault_account = &account_keys[account_index];
                    is_follow_vault(vault_account).map_or(None, |(pool_id, dex_type)| {
                        Some(Self {
                            dex_type,
                            pool_id,
                            account_index,
                            vault_account: vault_account.clone(),
                            post_account: post_amount.amount.clone(),
                            change_value: post_amount.ui_amount.sub(pre_amount.ui_amount),
                        })
                    })
                }
            }
            _ => None,
        }
    }

    fn new_from_one(
        tx: &[u8],
        is_pre: bool,
        one: &TokenBalance,
        account_keys: &[Pubkey],
    ) -> Option<Self> {
        let account_index = one.account_index as usize;
        let vault_account = &account_keys[account_index];
        match one.ui_token_amount.as_ref() {
            None => None,
            Some(token_amount) => {
                is_follow_vault(vault_account).map_or(None, |(pool_id, dex_type)| {
                    let (post_amount, change_value) = if is_pre {
                        ("0".to_string(), -token_amount.ui_amount)
                    } else {
                        (token_amount.amount.clone(), token_amount.ui_amount)
                    };
                    Some(Self {
                        dex_type,
                        pool_id,
                        account_index,
                        vault_account: vault_account.clone(),
                        post_account: post_amount,
                        change_value,
                    })
                })
            }
        }
    }

    pub fn collect_balance_change_infos(
        tx: &[u8],
        message: Option<Message>,
        meta: TransactionStatusMeta,
    ) -> Option<Vec<BalanceChangeInfo>> {
        let account_keys = message
            .unwrap()
            .account_keys
            .into_iter()
            .chain(meta.loaded_writable_addresses)
            .chain(meta.loaded_readonly_addresses)
            .map(|v| Pubkey::try_from(v).unwrap())
            .collect::<Vec<_>>();
        if get_subscribed_accounts()
            .bitand(&account_keys.to_vec().into_iter().collect::<AHashSet<_>>())
            .iter()
            .next()
            .is_none()
        {
            return None;
        }
        let pre_token_balances = meta.pre_token_balances;
        let post_token_balances = meta.post_token_balances;
        let pre_indices = pre_token_balances
            .iter()
            .map(|t| t.account_index)
            .collect::<AHashSet<_>>();
        let post_indices = post_token_balances
            .iter()
            .map(|t| t.account_index)
            .collect::<AHashSet<_>>();
        // 交集
        let common_indices = pre_indices
            .intersection(&post_indices)
            .cloned()
            .collect::<Vec<_>>();
        // 差集：只在 pre 里有
        let only_in_pre = pre_indices
            .difference(&post_indices)
            .cloned()
            .collect::<Vec<_>>();
        // 差集：只在 post 里有
        let only_in_post = post_indices
            .difference(&pre_indices)
            .cloned()
            .collect::<Vec<_>>();
        let mut changed_balances = pre_token_balances
            .iter()
            .filter(|t| common_indices.contains(&t.account_index))
            .zip(
                post_token_balances
                    .iter()
                    .filter(|t| common_indices.contains(&t.account_index)),
            )
            .filter_map(|(pre, post)| BalanceChangeInfo::new(tx, &pre, &post, &account_keys))
            .collect::<Vec<_>>();
        if !only_in_pre.is_empty() {
            changed_balances.extend(
                pre_token_balances
                    .into_iter()
                    .filter_map(|t| {
                        if only_in_pre.contains(&t.account_index) {
                            BalanceChangeInfo::new_from_one(tx, true, &t, &account_keys)
                        } else {
                            None
                        }
                    })
                    .collect::<Vec<_>>(),
            );
        }
        if !only_in_post.is_empty() {
            changed_balances.extend(
                post_token_balances
                    .into_iter()
                    .filter_map(|t| {
                        if only_in_post.contains(&t.account_index) {
                            BalanceChangeInfo::new_from_one(tx, false, &t, &account_keys)
                        } else {
                            None
                        }
                    })
                    .collect::<Vec<_>>(),
            );
        }
        if changed_balances.is_empty() {
            None
        } else {
            Some(changed_balances)
        }
    }

    fn print_amount_diff(tx: &[u8], balances: &[BalanceChangeInfo]) {
        balances.iter().for_each(|info| {
            if info.dex_type == DexType::RaydiumAMM
                || info.dex_type == DexType::PumpFunAMM
                || info.dex_type == DexType::RaydiumCPMM
            {
                let cache_vault_amount = match get_account_data::<MintVault>(&info.vault_account) {
                    None => 0,
                    Some(amount) => amount.amount,
                };
                if info.pool_id.to_string().as_str()=="58oQChx4yWmvKdwLLZzBi4ChoCc2fqCUWBkwMihLYQo2"||info.pool_id.to_string().as_str()=="9ViX1VductEoC2wERTSp2TuDxXPwAf69aeET8ENPJpsN" {
                    warn!(
                    "Processor : tx : {:?}, Dex类型: {:?}, 池子: {:?}, 金库: {:?}, Tx金库余额: {:?}, 缓存金库余额: {:?}",
                    tx.to_base58(),
                    info.dex_type,
                    info.pool_id,
                    info.vault_account,
                    info.post_account,
                    cache_vault_amount,
                    );
                }
            }
        })
    }
}

impl Debug for BalanceChangeInfo {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        let mut formatter = f.debug_struct("BalanceChangeInfo");
        formatter.field("dex_type", &self.dex_type);
        formatter.field("pool_id", &self.pool_id);
        formatter.field("vault_account", &self.vault_account.to_string());
        formatter.field("change_value", &self.change_value.to_string());
        formatter.finish()
    }
}
