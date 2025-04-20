use arrayref::{array_ref, array_refs};
use base58::ToBase58;
use chrono::Local;
use futures_util::future::ok;
use futures_util::stream::FuturesUnordered;
use log::{error, info};
use raydium_amm::state::{AmmInfo, Fees, Loadable, StateData};
use raydium_clmm::sdk::big_num::U128;
use raydium_clmm::sdk::pool::{PoolState, RewardInfo, REWARD_NUM};
use raydium_clmm::sdk::tick_array::{TickArrayState, TickState, TICK_ARRAY_SIZE_USIZE};
use raydium_clmm::sdk::tick_math::get_tick_at_sqrt_price;
use raydium_clmm::sdk::tickarray_bitmap_extension::TickArrayBitmapExtension;
use raydium_clmm::sdk::utils::{deserialize_anchor_account, deserialize_anchor_bytes};
use raydium_clmm::sdk::tick_array_bit_map;
use serde::__private::de::Content::I32;
use serde::{Deserialize, Serialize};
use serde_diff::{Diff, SerdeDiff};
use solana_program::program_pack::Pack;
use solana_program::pubkey::Pubkey;
use solana_sdk::pubkey;
use spl_token::state::{Account, AccountState};
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
use yellowstone_grpc_proto::tonic::codegen::tokio_stream::{StreamExt, StreamMap};
use raydium_clmm::sdk::pool;

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
        .subscribe_with_request(Some(generate_amm_info_sub_field_request()))
        // .subscribe_with_request(None)
        .await.unwrap();
    let (_, mut mint_vault_stream) = client
        .as_mut()
        .unwrap()
        .subscribe_with_request(Some(generate_mint_vault_sub_request()))
        // .subscribe_with_request(None)
        .await.unwrap();
    let mut map = StreamMap::new();
    map.insert("amm_info", stream);
    map.insert("mint_vault", mint_vault_stream);
    info!("connect grpc successful!");
    loop {
        tokio::select! {
            update = map.next() =>{
                if let Some((_,data)) = update {
                    match data{
                       Ok(msg) => match msg.update_oneof {
                            Some(UpdateOneof::Account(account)) => {
                                let account_info = &account.account.unwrap();
                                let pubkey=&account_info.pubkey.to_base58();
                                 // let  pool_state = amm_info_from_sub_data_slice(account_info.data.as_slice());
                                // info!("txn : {:?}, account : {:?}, data:{:?}",
                                //     &account_info.txn_signature.to_owned().unwrap().to_base58(),pubkey,pool_state);
                                // let amm_info = AmmInfo::load_from_bytes(&account_info.data).unwrap();
                                let  x = &account_info.txn_signature.to_owned().unwrap().to_base58();
                                info!("txn : {:?}, account : {:?}, data:{:?}",
                                    &account_info.txn_signature.to_owned().unwrap().to_base58(),
                                    pubkey,
                                    account_info.data
                                );
                            },
                            _=>{}
                        },
                        _=>{}
                    }

                }
            },
            // update = mint_vault_stream.next() =>{
            //     if let Some(data) = update {
            //         match data{
            //            Ok(msg) => match msg.update_oneof {
            //                 Some(UpdateOneof::Account(account)) => {
            //                     let account_info = &account.account.unwrap();
            //                     let txn = &account_info.txn_signature;
            //                     let pubkey=&account_info.pubkey.to_base58();
            //                     let account = mint_vault_from_sub_data_slice(&account_info.data);
            //                     info!("txn : {:?},pubkey:{:?}, data : {:?}",txn.to_owned().unwrap().to_base58(),pubkey,account);
            //                 },
            //                 _=>{}
            //             },
            //             _=>{}
            //         }
            //     }
            // },
        }
    }
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

fn generate_amm_info_sub_field_request() -> SubscribeRequest {
    let mut sub_accounts: HashMap<String, SubscribeRequestFilterAccounts> = HashMap::new();
    sub_accounts.insert(
        "58oQChx4yWmvKdwLLZzBi4ChoCc2fqCUWBkwMihLYQo2-account".to_string(),
        SubscribeRequestFilterAccounts {
            account: vec!["58oQChx4yWmvKdwLLZzBi4ChoCc2fqCUWBkwMihLYQo2".to_string()],
            ..Default::default()
        },
    );
    SubscribeRequest {
        accounts: sub_accounts,
        commitment: Some(CommitmentLevel::Processed).map(|x| x as i32),
        accounts_data_slice: vec![
            // fees.swap_fee_numerator
            SubscribeRequestAccountsDataSlice {
                offset: 176,
                length: 8,
            },
            // fees.swap_fee_denominator
            SubscribeRequestAccountsDataSlice {
                offset: 184,
                length: 8,
            },
            // state_data.need_take_pnl_coin
            SubscribeRequestAccountsDataSlice {
                offset: 192,
                length: 8,
            },
            // state_data.need_take_pnl_pc
            SubscribeRequestAccountsDataSlice {
                offset: 200,
                length: 8,
            },
        ],
        ..Default::default()
    }
}

fn amm_info_from_sub_data_slice(data: &[u8]) -> AmmInfo {
    let src = array_ref![data, 0, 32];
    let (swap_fee_numerator, swap_fee_denominator, need_take_pnl_coin, need_take_pnl_pc) =
        array_refs![src, 8, 8, 8, 8];
    AmmInfo {
        fees: Fees {
            swap_fee_numerator: u64::from_le_bytes(*swap_fee_numerator),
            swap_fee_denominator: u64::from_le_bytes(*swap_fee_denominator),
            ..Default::default()
        },
        state_data: StateData {
            need_take_pnl_coin: u64::from_le_bytes(*need_take_pnl_coin),
            need_take_pnl_pc: u64::from_le_bytes(*need_take_pnl_pc),
            ..Default::default()
        },
        ..Default::default()
    }
}

fn generate_mint_vault_sub_request() -> SubscribeRequest {
    let mut account_map: HashMap<String, SubscribeRequestFilterAccounts> = HashMap::new();
    account_map.insert(
        "DQyrAcCrDXQ7NeoqGgDCZwBvWDcYmFCjSb9JtteuvPpz-account".to_string(),
        SubscribeRequestFilterAccounts {
            account: vec!["DQyrAcCrDXQ7NeoqGgDCZwBvWDcYmFCjSb9JtteuvPpz".to_string()],
            ..Default::default()
        },
    );
    account_map.insert(
        "HLmqeL62xR1QoZ1HKKbXRrdN1p3phKpxRMb2VVopvBBz-account".to_string(),
        SubscribeRequestFilterAccounts {
            account: vec!["HLmqeL62xR1QoZ1HKKbXRrdN1p3phKpxRMb2VVopvBBz".to_string()],
            ..Default::default()
        },
    );
    SubscribeRequest {
        accounts: account_map,
        commitment: Some(CommitmentLevel::Processed).map(|x| x as i32),
        accounts_data_slice: vec![
            // mint
            SubscribeRequestAccountsDataSlice {
                offset: 0,
                length: 32,
            },
            // amount
            SubscribeRequestAccountsDataSlice {
                offset: 64,
                length: 8,
            },
            // amount
            SubscribeRequestAccountsDataSlice {
                offset: 108,
                length: 1,
            },
        ],
        ..Default::default()
    }
}

fn mint_vault_from_sub_data_slice(data: &[u8]) -> Account {
    let src = array_ref![data, 0, 41];
    let (mint, amount, state) = array_refs![src, 32, 8, 1];
    Account {
        mint: Pubkey::from(mint.to_owned()),
        amount: u64::from_le_bytes(*amount),
        state: AccountState::try_from(state[0]).unwrap(),
        ..Default::default()
    }
}

#[test]
fn calac_pool_state_sub_field_offset() {
    // 计算每个字段的offset和size
    let offsets = [
        (
            "coin_vault_mint",
            offset_of!(AmmInfo, coin_vault_mint),
            size_of::<Pubkey>(),
        ),
        (
            "pc_vault_mint",
            offset_of!(AmmInfo, pc_vault_mint),
            size_of::<Pubkey>(),
        ),
        (
            "fees.swap_fee_numerator",
            offset_of!(AmmInfo, fees.swap_fee_numerator),
            size_of::<u64>(),
        ),
        (
            "fees.swap_fee_denominator",
            offset_of!(AmmInfo, fees.swap_fee_denominator),
            size_of::<u64>(),
        ),
        (
            "state_data.need_take_pnl_coin",
            offset_of!(AmmInfo, state_data.need_take_pnl_coin),
            size_of::<u64>(),
        ),
        (
            "state_data.need_take_pnl_pc",
            offset_of!(AmmInfo, state_data.need_take_pnl_pc),
            size_of::<u64>(),
        ),
    ];

    // 打印结果
    for (field, offset, size) in offsets {
        println!(
            "AmmInfo \nField: {}, Offset: {} bytes, Size: {} bytes",
            field, offset, size
        );
    }
}

#[test]
fn calac_mint_vault_sub_field_offset() {
    // 计算每个字段的offset和size
    let offsets = [
        ("mint", offset_of!(Account, mint), size_of::<Pubkey>()),
        ("amount", offset_of!(Account, amount), size_of::<u64>()),
        (
            "state",
            offset_of!(Account, state),
            size_of::<AccountState>(),
        ),
    ];

    // 打印结果
    for (field, offset, size) in offsets {
        println!(
            "mint vault \nField: {}, Offset: {} bytes, Size: {} bytes",
            field, offset, size
        );
    }
}
