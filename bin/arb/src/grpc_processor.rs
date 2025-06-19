use crate::arb::Arb;
use crate::dex::meteora_damm_v2::state::pool::Pool;
use crate::dex::oracle::Oracle;
use crate::dex::tick_array::TickArray;
use crate::dex::whirlpool::Whirlpool;
use crate::dex::{
    get_account_data, get_dex_type_and_account_type, is_follow_vault, read_from, update_cache,
    AccountType, AmmInfo, BinArray, BinArrayBitmapExtension, LbPair, MintVault, PoolState,
    TickArrayBitmapExtension, TickArrayState, CLOCK_ID,
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
use std::fmt::{Debug, Formatter};
use std::ops::Sub;
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
            static COUNT: AtomicUsize = AtomicUsize::new(0);
            join_set.spawn(async move {
                loop {
                    match grpc_message_receiver.recv_async().await {
                        Ok(grpc_message) => match grpc_message {
                            GrpcMessage::Account(account_msg) => {
                                let c = COUNT.fetch_add(1, Ordering::Relaxed);
                                let log_info=if c % 500 == 0 {
                                    let tx = account_msg.tx.as_slice().to_base58();
                                    let account_key = Pubkey::try_from(
                                        account_msg.account_key.as_slice(),
                                    )
                                        .unwrap();
                                    warn!("Processor接收到Account， tx : {:?} , account_key : {:?}",
                                        tx, account_key
                                    );
                                    Some((tx, account_key))
                                } else {
                                    None
                                };
                                match Self::update_cache(
                                    account_msg.owner_key,
                                    account_msg.account_key,
                                    account_msg.data,
                                ) {
                                    Ok(_) => {
                                        if let Some((tx,acc))=log_info{
                                            warn!("Processor更新Account缓存成功， tx : {:?} , account_key : {:?}",
                                                tx, acc
                                            );
                                        }
                                    }
                                    Err(e) => {
                                        error!("更新缓存失败，{}", e);
                                    }
                                }
                            }
                            GrpcMessage::Transaction(transaction_msg) => {
                                let c = COUNT.fetch_add(1, Ordering::Relaxed);
                                let log_info=if c % 500 == 0 {
                                    let tx = transaction_msg.signature.as_slice().to_base58();
                                    warn!("Processor接收到Tx， tx : {:?}",tx);
                                    Some(tx)
                                }else {
                                    None
                                };
                                // #[cfg(feature = "print_data_after_update")]
                                // if let Some(changed_balances)=BalanceChangeInfo::collect_balance_change_infos(
                                //     transaction_msg.signature.as_slice(),
                                //     transaction_msg.transaction.as_ref().unwrap().message.clone(),
                                //     transaction_msg.meta.clone().unwrap(),
                                // ){
                                //     BalanceChangeInfo::print_amount_diff(transaction_msg.signature.as_slice(), changed_balances.as_slice());
                                // }
                                match cached_message_sender.try_send(transaction_msg) {
                                    Err(TrySendError::Full(msg)) => {
                                        cached_msg_drop_receiver.try_recv().ok();
                                        // info!("Processor_{index} Channel丢弃消息");
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
                                        if let Some(tx)=log_info{
                                            warn!("Processor发送Tx成功， tx : {:?}",tx);
                                        }
                                    }
                                }
                            }
                        },
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
                        // info!(
                        //     "balance, tx : {:?}, dex : {:?} , pool_id : {:?}, vault : {:?} , amount : {:?}",
                        //     tx.to_base58(),dex_type, pool_id, vault_account, post_amount.amount
                        // );
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
                    // info!(
                    //         "balance, tx : {:?}, dex : {:?} , pool_id : {:?}, vault : {:?} , amount : {:?}",
                    //         tx.to_base58(),dex_type, pool_id, vault_account, post_amount
                    //     );
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

// fn print_data_from_cache(
//     tx: &[u8],
//     owner: &Pubkey,
//     account_key: &Pubkey,
//     grpc_data: Vec<u8>,
// ) -> anyhow::Result<()> {
//     if account_key == &CLOCK_ID {
//         return Ok(());
//     }
//     let grpc_data = grpc_data.as_slice();
//     let diff_info = match get_dex_type_and_account_type(&owner, &account_key) {
//         None => Err(anyhow!("owner:{owner},account:{account_key}无法确定类型")),
//         Some((dex_type, account_type)) => match dex_type {
//             DexType::RaydiumAMM => match account_type {
//                 AccountType::Pool => Ok((
//                     DexType::RaydiumAMM,
//                     AccountType::Pool,
//                     get_diff::<AmmInfo, crate::dex::raydium_amm::old_state::pool::AmmInfo>(
//                         account_key,
//                         grpc_data,
//                     ),
//                 )),
//                 AccountType::MintVault => {
//                     Ok((
//                         DexType::RaydiumAMM,
//                         AccountType::MintVault,
//                         get_diff::<MintVault, Account>(account_key, grpc_data),
//                     ))
//                 },
//                 _ => Err(anyhow!("RaydiumAMM")),
//             },
//             DexType::RaydiumCLMM => match account_type {
//                 AccountType::Pool => Ok((
//                     DexType::RaydiumCLMM,
//                     AccountType::Pool,
//                     get_diff::<PoolState, crate::dex::raydium_clmm::old_state::pool::PoolState>(
//                         account_key,
//                         &grpc_data[8..],
//                     ),
//                 )),
//                 AccountType::TickArray => Ok((
//                     DexType::RaydiumCLMM,
//                     AccountType::TickArray,
//                     get_diff::<
//                         TickArrayState,
//                         crate::dex::raydium_clmm::old_state::tick_array::TickArrayState,
//                     >(account_key, &grpc_data[8..]),
//                 )),
//                 AccountType::TickArrayBitmap =>Ok((DexType::RaydiumCLMM, AccountType::TickArrayBitmap, get_diff::<
//                     TickArrayBitmapExtension,
//                     crate::dex::raydium_clmm::old_state::bitmap_extension::TickArrayBitmapExtension,
//                 >(account_key, &grpc_data[8..])))
//                 ,
//                 _ => Err(anyhow!("RaydiumCLMM")),
//             },
//             DexType::PumpFunAMM => match account_type {
//                 AccountType::MintVault =>{
//                     Ok((DexType::PumpFunAMM, AccountType::MintVault, get_diff::<MintVault, Account>(account_key, grpc_data)))
//                 }
//                    ,
//                 _ => Err(anyhow!("PumpFunAMM")),
//             },
//             DexType::MeteoraDLMM => match account_type {
//                 AccountType::Pool =>Ok((DexType::MeteoraDLMM, AccountType::Pool,  get_diff::<
//                     LbPair,
//                     crate::dex::meteora_dlmm::old_state::pool::LbPair,
//                 >(account_key, &grpc_data[8..])))
//                ,
//                 AccountType::BinArray => Ok((DexType::MeteoraDLMM, AccountType::BinArray,  get_diff::<
//                     BinArray,
//                     crate::dex::meteora_dlmm::old_state::bin_array::BinArray,
//                 >(account_key, &grpc_data[8..])))
//                 ,
//                 AccountType::BinArrayBitmap =>Ok((DexType::MeteoraDLMM, AccountType::BinArrayBitmap,  get_diff::<
//                     BinArrayBitmapExtension,
//                     crate::dex::meteora_dlmm::old_state::bitmap_extension::BinArrayBitmapExtension,
//                 >(account_key, &grpc_data[8..])))
//                 ,
//                 _ => Err(anyhow!("MeteoraDLMM")),
//             },
//             DexType::OrcaWhirl => match account_type {
//                 AccountType::Pool =>  Ok((DexType::OrcaWhirl, AccountType::Pool, get_diff::<
//                     Whirlpool,
//                     crate::dex::orca_whirlpools::old_state::pool::Whirlpool,
//                 >(account_key, grpc_data)))
//                 ,
//                 AccountType::TickArray =>Ok((DexType::OrcaWhirl, AccountType::TickArray,  get_diff::<
//                     TickArray,
//                     crate::dex::orca_whirlpools::old_state::tick_array::TickArray,
//                 >(account_key, grpc_data)))
//                 ,
//                 AccountType::Oracle =>Ok((DexType::OrcaWhirl, AccountType::Oracle,  get_diff::<
//                     Oracle,
//                     crate::dex::orca_whirlpools::old_state::oracle::Oracle,
//                 >(account_key, grpc_data)))
//                 ,
//                 _ => Err(anyhow!("OrcaWhirl")),
//             },
//             DexType::MeteoraDAMMV2 => match account_type {
//                 AccountType::Pool => Ok((DexType::MeteoraDAMMV2, AccountType::Pool, get_diff::<
//                     Pool,
//                     crate::dex::meteora_damm_v2::old_state::pool::Pool,
//                 >(account_key, &grpc_data[8..])))
//                 ,
//                 _ => Err(anyhow!("MeteoraDAMMV2")),
//             },
//             DexType::RaydiumCPMM => match account_type {
//                 AccountType::Pool =>Ok((DexType::RaydiumCPMM, AccountType::Pool,  get_diff::<
//                     crate::dex::raydium_cpmm::states::PoolState,
//                     crate::dex::raydium_cpmm::old_state::pool::PoolState,
//                 >(account_key, &grpc_data[8..])))
//                 ,
//                 AccountType::MintVault =>Ok((DexType::RaydiumCPMM, AccountType::MintVault,  get_diff::<MintVault, Account>(account_key, grpc_data)))
//                     ,
//                 _ => Err(anyhow!("RaydiumCPMM")),
//             },
//         },
//     };
//     match diff_info {
//         Ok((dex_type, account_type, diff)) => match diff {
//             Ok(d) => {
//                 if let Some(df) = d {
//                     info!(
//                         "{:?} {:?}, key : {:?}\n{:?}",
//                         dex_type,
//                         account_type,
//                         account_key,
//                         serde_json::to_string(&df)
//                     );
//                 }
//                 // else {
//                 //     info!("{:?}一致", account_key);
//                 // }
//             }
//             Err(e) => {
//                 error!(
//                     "dex:{dex_type},account_type:{:?},account_key:{account_key},错误：{e}",
//                     account_type
//                 );
//             }
//         },
//         Err(e) => {
//             error!("owner:{owner},account_key:{account_key},错误：{e}",);
//         }
//     }
//     Ok(())
// }

fn get_diff<Taget: FromCache + Serialize + Debug, F: TryInto<Taget> + Debug>(
    account_key: &Pubkey,
    grpc_data: &[u8],
) -> anyhow::Result<Option<Difference>>
where
    <F as TryInto<Taget>>::Error: Debug,
{
    if grpc_data.len() == 0 {
        return Ok(None);
    }
    let origin_pool = unsafe { read_from::<F>(grpc_data) };
    let a = origin_pool
        .try_into()
        .map_err(|e| anyhow!("grpc数据转换失败，原因：{:?}", e))?;
    // info!("a:{:#?}", a);
    let custom_pool = get_account_data::<Taget>(&account_key).map_or(
        Err(anyhow!("account:{account_key}缓存中没有")),
        |data| Ok(data),
    )?;
    // info!("custom_pool:{:#?}", custom_pool);
    // 序列化为 serde_json::Value
    let old_json = serde_json::to_value(&custom_pool)?;
    let new_json = serde_json::to_value(&a)?;
    let result =
        serde_json_diff::values(old_json, new_json).map_or(Ok(None), |diff| Ok(Some(diff)));
    result
}
