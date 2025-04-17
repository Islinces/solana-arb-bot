use arrayref::{array_ref, array_refs};
use base58::ToBase58;
use chrono::Local;
use futures_util::future::ok;
use futures_util::stream::FuturesUnordered;
use log::{error, info};
use raydium_amm::state::{AmmInfo, Loadable};
use raydium_clmm::big_num::U128;
use raydium_clmm::pool::{PoolState, RewardInfo, REWARD_NUM};
use raydium_clmm::tick_array::{TickArrayState, TickState, TICK_ARRAY_SIZE_USIZE};
use raydium_clmm::tick_math::get_tick_at_sqrt_price;
use raydium_clmm::tickarray_bitmap_extension::TickArrayBitmapExtension;
use raydium_clmm::utils::{deserialize_anchor_account, deserialize_anchor_bytes};
use raydium_clmm::{pool, tick_array_bit_map};
use serde::__private::de::Content::I32;
use serde::{Deserialize, Serialize};
use serde_diff::{Diff, SerdeDiff};
use solana_program::pubkey::Pubkey;
use solana_sdk::pubkey;
use std::collections::HashMap;
use std::env;
use std::io::Write;
use std::mem::offset_of;
use std::ops::Sub;
use std::process::exit;
use std::time::Duration;
use yellowstone_grpc_client::{GeyserGrpcClient, GeyserGrpcClientResult, Interceptor};
use yellowstone_grpc_proto::geyser::subscribe_update::UpdateOneof;
use yellowstone_grpc_proto::geyser::{
    CommitmentLevel, SubscribeRequest, SubscribeRequestAccountsDataSlice,
    SubscribeRequestFilterAccounts, SubscribeRequestFilterTransactions, SubscribeUpdateAccount,
};
use yellowstone_grpc_proto::tonic::codegen::tokio_stream::StreamExt;

#[test]
fn test() {
    println!(
        "name:{:?}",
        [
            43, 49, 147, 96, 191, 162, 206, 203, 146, 196, 176, 164, 96, 64, 154, 15, 4, 40, 3, 63,
            96, 19, 0, 184, 179, 252, 249, 156, 243, 96, 46, 135,
        ]
        .to_base58()
    );
    let slice: [u8; 16] = [181, 198, 153, 183, 27, 156, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0];
    let u129 = U128::from_little_endian(slice.as_slice()).as_u128();
    println!("price : {:?}", u129);
    println!("price : {:?}", u128::from_le_bytes(slice));
    println!("price : {:?}", i32::from_be_bytes([174, 175, 255, 255]));
    println!(
        "price : {:?}",
        [
            37, 174, 48, 152, 88, 209, 141, 133, 119, 93, 77, 173, 145, 178, 249, 150, 62, 61, 105,
            219, 165, 61, 79, 226, 101, 182, 105, 158, 71, 91, 177, 25
        ]
        .to_base58()
    );
}

#[test]
fn calac_pool_state_sub_field_offset() {
    // 计算每个字段的offset和size
    let offsets = [
        (
            "tick_spacing",
            offset_of!(PoolState, tick_spacing) as usize,
            size_of::<u16>(),
        ),
        (
            "liquidity",
            offset_of!(PoolState, liquidity) as usize,
            size_of::<u128>(),
        ),
        (
            "sqrt_price_x64",
            offset_of!(PoolState, sqrt_price_x64) as usize,
            size_of::<u128>(),
        ),
        (
            "tick_current",
            offset_of!(PoolState, tick_current) as usize,
            size_of::<i32>(),
        ),
        (
            "tick_array_bitmap",
            offset_of!(PoolState, tick_array_bitmap) as usize,
            size_of::<[u64; 16]>(),
        ),
        (
            "total_fees_token_0",
            offset_of!(PoolState, total_fees_token_0) as usize,
            size_of::<u64>(),
        ),
        (
            "total_fees_token_1",
            offset_of!(PoolState, total_fees_token_1) as usize,
            size_of::<u64>(),
        ),
    ];

    // 打印结果
    for (field, offset, size) in offsets {
        println!(
            "PoolState \nField: {}, Offset: {} bytes, Size: {} bytes",
            field,
            offset + 8,
            size
        );
    }
}

#[test]
fn calac_tick_array_state_field_offset() {
    // 计算每个字段的offset和size
    let offsets = [
        (
            "pool_id",
            offset_of!(TickArrayState, pool_id) as usize,
            size_of::<Pubkey>(),
        ),
        (
            "start_tick_index",
            offset_of!(TickArrayState, start_tick_index) as usize,
            size_of::<i32>(),
        ),
        (
            "ticks",
            offset_of!(TickArrayState, ticks) as usize,
            size_of::<[TickState; TICK_ARRAY_SIZE_USIZE]>(),
        ),
        (
            "initialized_tick_count",
            offset_of!(TickArrayState, initialized_tick_count) as usize,
            size_of::<u8>(),
        ),
    ];

    for (field, offset, size) in offsets {
        println!(
            "TickArrayState \nField: {}, Offset: {} bytes, Size: {} bytes",
            field, offset, size
        );
    }
    //TickState
    let offsets = [
        (
            "tick",
            offset_of!(TickState, tick) as usize,
            size_of::<i32>(),
        ),
        (
            "liquidity_gross",
            offset_of!(TickState, liquidity_gross) as usize,
            size_of::<u128>(),
        ),
        (
            "fee_growth_outside_0_x64",
            offset_of!(TickState, fee_growth_outside_0_x64) as usize,
            size_of::<u128>(),
        ),
        (
            "fee_growth_outside_1_x64",
            offset_of!(TickState, fee_growth_outside_1_x64) as usize,
            size_of::<u128>(),
        ),
    ];

    // 打印结果
    for (field, offset, size) in offsets {
        println!(
            "TickState \nField: {}, Offset: {} bytes, Size: {} bytes",
            field,
            offset + 36,
            size
        );
    }
}

#[tokio::test]
async fn main() {
    env::set_var(
        env_logger::DEFAULT_FILTER_ENV,
        env::var_os(env_logger::DEFAULT_FILTER_ENV).unwrap_or_else(|| "info".into()),
    );
    env_logger::builder()
        .format(|buf, record| {
            write!(
                buf,
                "{} [{}] - {}\n",
                Local::now().format("%Y-%m-%d %H:%M:%S%.6f"),
                record.level(),
                record.args()
            )
        })
        .init();

    let _ = sub().await;
}

async fn sub() -> anyhow::Result<()> {
    let grpc_client = client();
    let mut client = grpc_client.await;
    if client.is_err() {
        return Err(anyhow::Error::msg("failed to connect"));
    }
    let (_subscribe_tx, mut stream) = client
        .as_mut()
        .unwrap()
        .subscribe_with_request(Some(generate_pool_state_sub_field_request()))
        // .subscribe_with_request(None)
        .await?;
    let (_, mut tick_array_stream) = client
        .as_mut()
        .unwrap()
        .subscribe_with_request(Some(generate_tick_array_state_sub_request()))
        // .subscribe_with_request(None)
        .await?;
    let (_, mut tick_array_bitmap_extension_stream) = client
        .as_mut()
        .unwrap()
        .subscribe_with_request(Some(generate_tick_array_bitmap_extension_sub_request()))
        // .subscribe_with_request(None)
        .await?;

    loop {
        tokio::select! {
            update = stream.next() =>{
                if let Some(data) = update {
                    match data{
                       Ok(msg) => match msg.update_oneof {
                            Some(UpdateOneof::Account(account)) => {
                                let account_info = &account.account.unwrap();
                                let src=array_ref![account_info.data.as_slice(),0,180];
                                let pubkey=&account_info.pubkey.to_base58();
                                 let  pool_state = pool_state_from_sub_data_slice(account_info.data.as_slice());
                                info!("txn : {:?}, account : {:?}, data:{:?}",
                                    &account_info.txn_signature.to_owned().unwrap().to_base58(),pubkey,pool_state);
                            },
                            _=>{}
                        },
                        _=>{}
                    }

                }
            },
            update = tick_array_stream.next() =>{
                if let Some(data) = update {
                    match data{
                       Ok(msg) => match msg.update_oneof {
                            Some(UpdateOneof::Account(account)) => {
                                let account_info = &account.account.unwrap();
                                let txn = &account_info.txn_signature;
                                let pubkey=&account_info.pubkey.to_base58();
                                // info!("txn : {:?},pubkey:{:?} data : {:?}",txn.to_owned().unwrap().to_base58(),pubkey,&account_info.data);
                                let  tick_array_state = tick_array_state_from_sub_data_slice(&account_info.data);
                                info!("txn : {:?}, tick_array_state_pubkey : {:?}, data:{:?}",txn.to_owned().unwrap().to_base58(),pubkey,tick_array_state);
                            },
                            _=>{}
                        },
                        _=>{}
                    }
                }
            },
            update = tick_array_bitmap_extension_stream.next() =>{
                if let Some(data) = update {
                    match data{
                       Ok(msg) => match msg.update_oneof {
                            Some(UpdateOneof::Account(account)) => {
                                let account_info = &account.account.unwrap();
                                let txn = &account_info.txn_signature;
                                let pubkey=&account_info.pubkey.to_base58();
                                let  tick_array_state = tick_array_bitmap_extension_from_sub_data_slice(&account_info.data);
                                info!("txn : {:?}, tick_array_extension_bitmap_pubkey : {:?}, data:{:?}",txn.to_owned().unwrap().to_base58(),pubkey,tick_array_state);
                            },
                            _=>{}
                        },
                        _=>{}
                    }
                }
            }
        }
    }
}

fn calcate_need_wait_account_count(
    pool_cache: &mut HashMap<String, PoolState>,
    msg: &SubscribeUpdateAccount,
) -> (bool, usize) {
    let account_data = msg.account.as_ref().unwrap();
    let pubkey = &account_data.pubkey.to_base58();
    let current_pool_state =
        deserialize_anchor_bytes::<PoolState>(&account_data.data.as_slice()).unwrap();
    let pool_state_from_cache = pool_cache.get(pubkey);
    match pool_state_from_cache {
        None => {
            pool_cache.insert(pubkey.clone(), current_pool_state);
            (false, 0)
        }
        Some(cache_data) => {
            let is_swap = is_swap_instruction(cache_data, &current_pool_state);
            (false, 0)
        }
    }
}
/// TODO：需要根据日志再确认下
fn is_swap_instruction(old: &PoolState, new: &PoolState) -> (bool, bool, Option<usize>) {
    let is_swap = old.tick_current != new.tick_current
        || (old.sqrt_price_x64 != new.sqrt_price_x64
            && (old.total_fees_token_1 != new.total_fees_token_1
                || old.total_fees_token_0 != new.total_fees_token_0));
    if !is_swap {
        (false, false, None)
    } else {
        (
            true,
            // 交易方向
            old.total_fees_token_0 != new.total_fees_token_0,
            // 影响的TickArrayState数量
            if new.tick_current == old.tick_current || old.liquidity == new.liquidity {
                Some(0)
            } else {
                // swap：tick_current变更，liquidity也会变化
                // swap_v2: tick_current变化，liquidity不一定变化，反之，liquidity变化，tick_current必定变化
                Some(
                    TickArrayState::get_array_start_index(old.tick_current, new.tick_spacing)
                        .sub(TickArrayState::get_array_start_index(
                            new.tick_current,
                            new.tick_spacing,
                        ))
                        .abs() as usize,
                )
            },
        )
    }
}

/// TODO：需要根据日志再确认下
fn is_decrease_liquidity(old: &PoolState, new: &PoolState) -> bool {
    old.total_fees_claimed_token_1 != new.total_fees_claimed_token_1
        && old.total_fees_claimed_token_0 != new.total_fees_claimed_token_0
}

async fn client() -> anyhow::Result<GeyserGrpcClient<impl Interceptor>> {
    let mut builder =
        GeyserGrpcClient::build_from_static("https://solana-yellowstone-grpc.publicnode.com");
    builder = builder
        .tcp_nodelay(true)
        .http2_adaptive_window(true)
        .buffer_size(65536)
        .initial_connection_window_size(5242880)
        .initial_stream_window_size(4194304)
        .connect_timeout(Duration::from_millis(10 * 1000));
    builder.connect().await.map_err(|e| {
        error!("failed to connect: {e}");
        anyhow::anyhow!(e)
    })
}

fn generate_pool_state_sub_field_request() -> SubscribeRequest {
    let mut sub_accounts: HashMap<String, SubscribeRequestFilterAccounts> = HashMap::new();
    sub_accounts.insert(
        "3ucNos4NbumPLZNWztqGHNFFgkHeRMBQAVemeeomsUxv-account".to_string(),
        SubscribeRequestFilterAccounts {
            account: vec!["3ucNos4NbumPLZNWztqGHNFFgkHeRMBQAVemeeomsUxv".to_string()],
            ..Default::default()
        },
    );
    SubscribeRequest {
        accounts: sub_accounts,
        commitment: Some(CommitmentLevel::Processed).map(|x| x as i32),
        accounts_data_slice: vec![
            // liquidity
            SubscribeRequestAccountsDataSlice {
                offset: 237,
                length: 16,
            },
            // sqrt_price_x64
            SubscribeRequestAccountsDataSlice {
                offset: 253,
                length: 16,
            },
            // tick_current
            SubscribeRequestAccountsDataSlice {
                offset: 269,
                length: 4,
            },
            // tick_array_bitmap
            SubscribeRequestAccountsDataSlice {
                offset: 904,
                length: 128,
            },
            // total_fees_token_0
            SubscribeRequestAccountsDataSlice {
                offset: 1032,
                length: 8,
            },
            // total_fees_token_1
            SubscribeRequestAccountsDataSlice {
                offset: 1048,
                length: 8,
            },
        ],
        ..Default::default()
    }
}

fn pool_state_from_sub_data_slice(data: &[u8]) -> PoolState {
    let src = array_ref![data, 0, 180];
    let (liquidity, price, tick_current, bitmap, total_fees_token_0, total_fees_token_1) =
        array_refs![src, 16, 16, 4, 128, 8, 8];
    PoolState {
        liquidity: u128::from_le_bytes(*liquidity),
        sqrt_price_x64: u128::from_le_bytes(*price),
        tick_current: i32::from_le_bytes(*tick_current),
        tick_array_bitmap: bitmap
            .chunks_exact(8)
            .map(|chunk| u64::from_le_bytes(chunk.try_into().unwrap()))
            .collect::<Vec<_>>()
            .try_into()
            .unwrap(),
        total_fees_token_0: u64::from_le_bytes(*total_fees_token_0),
        total_fees_token_1: u64::from_le_bytes(*total_fees_token_1),
        ..Default::default()
    }
}

fn generate_tick_array_state_sub_request() -> SubscribeRequest {
    let mut sub_tick_array_accounts: HashMap<String, SubscribeRequestFilterAccounts> =
        HashMap::new();
    sub_tick_array_accounts.insert(
        "82bNhYjhen5CRpoBmMhwY8WYfPsRTmjxup7sS4Lxo5aN-account".to_string(),
        SubscribeRequestFilterAccounts {
            account: vec![
                "82bNhYjhen5CRpoBmMhwY8WYfPsRTmjxup7sS4Lxo5aN".to_string(),
                "Fs6dvg4JN2kGvVmnLy8upR2fBCXApLzuaJTfg6KRZEtH".to_string(),
                "6TjJF9MPeGuQEthwHxtD7PXoUS8CqvXMvZDdD9fqrZua".to_string(),
                "EgLwEjYBpeBZEqmPoVxCA5xPRFh1ZgAVz16NYfrCjxZV".to_string(),
                "25HAifeWsVmLFWxV7BQt9eH9YFoxUpjoAYbJ37XX6b3Z".to_string(),
                "FPgKdPCeETaHvukpqGa55m5XmrCmDSrC8xVrYsHgSqjJ".to_string(),
                "ECiyVMnZvzzjZXkKBk79fNUpV66zFbcJJLuXW7i3qouf".to_string(),
                "3RPeqUwNexYi4hiVHyuaYv723XPAduLJdi9KUkUaHSUL".to_string(),
                "38w3hZq69BZZ8G5v5hM9JUHXzBQB24HvBZg8Gz85YQH1".to_string(),
                "J99LrLSvvvina6RWTcPu88ZN7MdcaV8WD91WL4i3dQHU".to_string(),
                "2HQa6tjCMzeV8uGG6BUf4jAzhHjQssJMZAn9U5aJbXXd".to_string(),
            ],
            ..Default::default()
        },
    );
    let mut filter = Vec::new();
    let mut start_index = 8;
    // pool_id
    filter.push(SubscribeRequestAccountsDataSlice {
        offset: start_index,
        length: 32,
    });
    start_index += 32;
    // start_tick_index
    filter.push(SubscribeRequestAccountsDataSlice {
        offset: start_index,
        length: 4,
    });
    start_index += 4;
    // ticks 60个
    for _ in 0..60 {
        // tick
        filter.push(SubscribeRequestAccountsDataSlice {
            offset: start_index,
            length: 4,
        });
        start_index += 4;
        // liquidity_net
        start_index += 16;
        // liquidity_gross
        filter.push(SubscribeRequestAccountsDataSlice {
            offset: start_index,
            length: 16,
        });
        start_index += 16;
        // fee_growth_outside_0_x64
        filter.push(SubscribeRequestAccountsDataSlice {
            offset: start_index,
            length: 16,
        });
        start_index += 16;
        // fee_growth_outside_1_x64
        filter.push(SubscribeRequestAccountsDataSlice {
            offset: start_index,
            length: 16,
        });
        start_index += 16;
        // reward_growths_outside_x64
        start_index += 16 * 3;
        // padding
        start_index += 52;
    }
    // initialized_tick_count
    filter.push(SubscribeRequestAccountsDataSlice {
        offset: 10116,
        length: 1,
    });
    SubscribeRequest {
        accounts: sub_tick_array_accounts,
        commitment: Some(CommitmentLevel::Processed).map(|x| x as i32),
        accounts_data_slice: filter,
        ..Default::default()
    }
}

fn tick_array_state_from_sub_data_slice(data: &Vec<u8>) -> TickArrayState {
    let src = array_ref![data.as_slice(), 0, 3157];
    let (pool_id, start_tick_index, ticks_data, initialized_tick_count) =
        array_refs![src, 32, 4, 3120, 1];
    let ticks: [TickState; 60] = ticks_data
        .chunks_exact(52)
        .map(|chunk| {
            let src = array_ref![chunk, 0, 52];
            let (tick, liquidity_gross, fee_growth_outside_0_x64, fee_growth_outside_1_x64) =
                array_refs![src, 4, 16, 16, 16];
            TickState {
                tick: i32::from_le_bytes(*tick),
                liquidity_gross: u128::from_le_bytes(*liquidity_gross),
                fee_growth_outside_0_x64: u128::from_le_bytes(*fee_growth_outside_0_x64),
                fee_growth_outside_1_x64: u128::from_le_bytes(*fee_growth_outside_1_x64),
                ..Default::default()
            }
        })
        .collect::<Vec<_>>()
        .try_into()
        .unwrap();
    TickArrayState {
        pool_id: Pubkey::from(pool_id.to_owned()),
        start_tick_index: i32::from_le_bytes(*start_tick_index),
        initialized_tick_count: u8::from_le_bytes(*initialized_tick_count),
        ticks,
        ..Default::default()
    }
}

fn generate_tick_array_bitmap_extension_sub_request() -> SubscribeRequest {
    let mut account_map: HashMap<String, SubscribeRequestFilterAccounts> = HashMap::new();
    account_map.insert(
        "4NFvUKqknMpoe6CWTzK758B8ojVLzURL5pC6MtiaJ8TQ-account".to_string(),
        SubscribeRequestFilterAccounts {
            account: vec!["4NFvUKqknMpoe6CWTzK758B8ojVLzURL5pC6MtiaJ8TQ".to_string()],
            ..Default::default()
        },
    );
    SubscribeRequest {
        accounts: account_map,
        commitment: Some(CommitmentLevel::Processed).map(|x| x as i32),
        ..Default::default()
    }
}

fn tick_array_bitmap_extension_from_sub_data_slice(data: &Vec<u8>) -> TickArrayBitmapExtension {
    deserialize_anchor_bytes::<TickArrayBitmapExtension>(data).unwrap()
}
