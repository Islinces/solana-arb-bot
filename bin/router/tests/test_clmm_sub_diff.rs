use base58::ToBase58;
use chrono::Local;
use futures_util::future::ok;
use futures_util::stream::FuturesUnordered;
use serde::{Deserialize, Serialize};
use serde_diff::{Diff, SerdeDiff};
use solana_program::pubkey::Pubkey;
use solana_sdk::pubkey;
use std::collections::HashMap;
use std::env;
use std::io::Write;
use std::ops::Sub;
use std::process::exit;
use std::time::Duration;
use tracing::{error, info};
use tracing_appender::non_blocking;
use tracing_appender::rolling::{RollingFileAppender, Rotation};
use tracing_subscriber::{fmt, EnvFilter};
use tracing_subscriber::fmt::format::FmtSpan;
use tracing_subscriber::fmt::time::FormatTime;
use tracing_subscriber::layer::SubscriberExt;
use tracing_subscriber::util::SubscriberInitExt;
use yellowstone_grpc_client::{GeyserGrpcClient, GeyserGrpcClientResult, Interceptor};
use yellowstone_grpc_proto::geyser::subscribe_update::UpdateOneof;
use yellowstone_grpc_proto::geyser::{
    CommitmentLevel, SubscribeRequest, SubscribeRequestAccountsDataSlice,
    SubscribeRequestFilterAccounts, SubscribeRequestFilterTransactions, SubscribeUpdateAccount,
};
use yellowstone_grpc_proto::tonic::codegen::tokio_stream::StreamExt;
use router::dex::common::utils::deserialize_anchor_bytes;
use router::dex::raydium_clmm::sdk::pool::PoolState;
use router::dex::raydium_clmm::sdk::tick_array::{TickArrayState, TickState};
use router::dex::raydium_clmm::sdk::tickarray_bitmap_extension::TickArrayBitmapExtension;

#[test]
fn test() {
    let slice: [u8; 16] = [0, 0, 0, 89, 91, 238, 255, 247, 15, 120, 90, 0, 0, 0, 0, 0];
    println!("price : {:?}", u128::from_le_bytes(slice));
}

pub struct MicrosecondFormatter;

impl FormatTime for MicrosecondFormatter {
    fn format_time(&self, w: &mut fmt::format::Writer<'_>) -> std::fmt::Result {
        write!(w, "{}", Local::now().format("%Y-%m-%d %H:%M:%S%.6f"))
    }
}

#[tokio::test]
async fn main() {
    let filter = EnvFilter::new("info")
        // .add_directive("router=debug".parse().unwrap())
        ;
    let file_appender = RollingFileAppender::builder()
        .filename_prefix("app")
        .filename_suffix("log")
        .rotation(Rotation::DAILY)
        .build("./logs")
        .expect("TODO: panic message");
    let (non_blocking_writer, _guard) = non_blocking(file_appender);
    tracing_subscriber::registry()
        .with(
            fmt::layer()
                .with_timer(MicrosecondFormatter)
                .with_writer(non_blocking_writer)
                .with_thread_ids(true)
                .with_thread_names(true)
                .with_span_events(FmtSpan::CLOSE),
        )
        .with(filter)
        .init();

    let _ = sub().await;
}

async fn sub() -> anyhow::Result<()> {
    let grpc_client = client();
    let mut transactions: HashMap<String, SubscribeRequestFilterTransactions> = HashMap::new();
    transactions.insert(
        "3ucNos4NbumPLZNWztqGHNFFgkHeRMBQAVemeeomsUxv-tx".to_string(),
        SubscribeRequestFilterTransactions {
            account_include: vec!["3ucNos4NbumPLZNWztqGHNFFgkHeRMBQAVemeeomsUxv".to_string()],
            failed: Some(false),
            vote: Some(false),
            ..Default::default()
        },
    );
    let mut sub_accounts: HashMap<String, SubscribeRequestFilterAccounts> = HashMap::new();
    sub_accounts.insert(
        "3ucNos4NbumPLZNWztqGHNFFgkHeRMBQAVemeeomsUxv-account".to_string(),
        SubscribeRequestFilterAccounts {
            account: vec![
                "3ucNos4NbumPLZNWztqGHNFFgkHeRMBQAVemeeomsUxv".to_string(),
                "4NFvUKqknMpoe6CWTzK758B8ojVLzURL5pC6MtiaJ8TQ".to_string(),
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
            // owner: vec![
            //     "675kPX9MHTjS2zt1qfr1NYHuzeLXfQM9H24wFSUt1Mp8".to_string(),
            //     "5Q544fKrFoe6tsEbD7S8EmxGTJYAKtTVhAW5Q5pge4j1".to_string(),
            // ],
            ..Default::default()
        },
    );
    let request = SubscribeRequest {
        transactions,
        accounts: sub_accounts,
        commitment: Some(CommitmentLevel::Processed).map(|x| x as i32),
        // accounts_data_slice: vec![SubscribeRequestAccountsDataSlice {
        //     offset: 253,
        //     length: 16,
        // }],
        ..Default::default()
    };
    let result = grpc_client.await;
    if result.is_err() {
        return Err(anyhow::Error::msg("failed to connect"));
    }
    let sub_result = result?.subscribe_with_request(Some(request)).await;
    if sub_result.is_err() {
        return Err(anyhow::Error::msg("failed to sub"));
    }

    let (_subscribe_tx, mut stream) = sub_result?;
    let mut previous_tick_index = None;
    let mut pool_cache: HashMap<String, PoolStateDiff> = HashMap::new();
    let mut bitmap_extension_cache: HashMap<String, TickArrayBitmapExtensionDiff> = HashMap::new();
    let mut tick_state_cache: HashMap<String, TickArrayStateDiff> = HashMap::new();
    loop {
        tokio::select! {
            update = stream.next() =>{
                if let Some(data) = update {
                    match data{
                       Ok(msg) => match msg.update_oneof {
                            Some(UpdateOneof::Transaction(msg)) => {
                                let tx = msg
                                    .transaction
                                    .ok_or(anyhow::anyhow ! ("no transaction in the message")) ?;
                                let result = & tx.signature.to_base58();
                                info ! ("tx  : {:#?},slot : {:?}", result,msg.slot);
                            },
                            Some(UpdateOneof::Account(account)) => {
                                // let state = deserialize_anchor_bytes::< pool::PoolState >(account.account.unwrap().data.as_slice()).unwrap();
                                // info ! ("pool_state : {:#?}", state.amm_config);
                                // let  amm_info = AmmInfo::load_from_bytes(x.data.as_slice()).unwrap();
                                let  account_info = &account.account.unwrap();
                                let  txn = &account_info.txn_signature;
                                let pubkey=&account_info.pubkey.to_base58();
                                let mut diff_json=None;
                                let mut tick_chaned=None;
                                match pubkey.as_str(){
                                    "3ucNos4NbumPLZNWztqGHNFFgkHeRMBQAVemeeomsUxv" =>{
                                        let state = deserialize_anchor_bytes::<PoolState>(account_info.data.as_slice()).unwrap();
                                        let current=state.tick_current;
                                        let previous= match previous_tick_index{
                                            None=>{
                                                previous_tick_index=Some(current);
                                                0
                                            },
                                            Some(previous)=>{
                                                previous_tick_index=Some(current);
                                                previous
                                            },
                                        };
                                        tick_chaned=Some((previous,current));
                                        let  pool_state_cache = pool_cache.get(pubkey);
                                        match pool_state_cache{
                                            None=>{
                                                pool_cache.insert(pubkey.to_string(),PoolStateDiff::from(&state));
                                            },
                                            Some(pool)=>{
                                                let state_diff=PoolStateDiff::from(&state);
                                                let diff = Diff::serializable(pool,&state_diff);
                                                let json_diff = serde_json::to_string(&diff).unwrap();
                                                diff_json=Some(json_diff);
                                                pool_cache.insert(pubkey.to_string(),state_diff);
                                            }
                                        }
                                    },
                                    "4NFvUKqknMpoe6CWTzK758B8ojVLzURL5pC6MtiaJ8TQ" =>{
                                        let state = deserialize_anchor_bytes::<TickArrayBitmapExtension>(account_info.data.as_slice()).unwrap();
                                        let  bitmap_cache = bitmap_extension_cache.get(pubkey);
                                        match bitmap_cache{
                                            None=>{
                                                bitmap_extension_cache.insert(pubkey.to_string(),TickArrayBitmapExtensionDiff::from(state));
                                            },
                                            Some(bitmap)=>{
                                                let state_diff=TickArrayBitmapExtensionDiff::from(state);
                                                let diff = Diff::serializable(bitmap,&state_diff);
                                                let json_diff = serde_json::to_string(&diff).unwrap();
                                                diff_json=Some(json_diff);
                                                bitmap_extension_cache.insert(pubkey.to_string(),state_diff);
                                            }
                                        }
                                    },
                                    _=>{
                                        let state = deserialize_anchor_bytes::<TickArrayState>(account_info.data.as_slice());
                                        if let Ok(tick_array_state) =state{
                                            let  tick_state = tick_state_cache.get(pubkey);
                                            match tick_state{
                                                None=>{
                                                    tick_state_cache.insert(pubkey.to_string(),TickArrayStateDiff::from(tick_array_state));
                                                },
                                                Some(tick)=>{
                                                    let state_diff=TickArrayStateDiff::from(tick_array_state);
                                                    let diff = Diff::serializable(tick,&state_diff);
                                                    let json_diff = serde_json::to_string(&diff).unwrap();
                                                    diff_json=Some(json_diff);
                                                    tick_state_cache.insert(pubkey.to_string(),state_diff);
                                                }
                                            }
                                        }
                                    }
                                }
                                info!("txn : {:?}, account : {:?}, tick_current changed : {:?}, diff_json : {:?}", txn.as_ref().unwrap().to_base58(),pubkey,tick_chaned,diff_json);
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

async fn client() -> anyhow::Result<GeyserGrpcClient<impl Interceptor>> {
    let builder =
        GeyserGrpcClient::build_from_static("https://solana-yellowstone-grpc.publicnode.com");
    // builder = builder
    //     .tcp_nodelay(true)
    //     .http2_adaptive_window(true)
    //     .buffer_size(65536)
    //     .initial_connection_window_size(5242880)
    //     .initial_stream_window_size(4194304)
    //     .connect_timeout(Duration::from_millis(10 * 1000));
    builder.connect().await.map_err(|e| {
        error!("failed to connect: {e}");
        anyhow::anyhow!(e)
    })
}

#[derive(Debug, SerdeDiff, Serialize, Deserialize)]
struct PoolStateDiff {
    /// The currently in range liquidity available to the pool.
    pub liquidity: u128,
    /// The current price of the pool as a sqrt(token_1/token_0) Q64.64 value
    pub sqrt_price_x64: u128,
    /// The current tick of the pool, i.e. according to the last tick transition that was run.
    pub tick_current: i32,
    pub fee_growth_global_0_x64: u128,
    pub fee_growth_global_1_x64: u128,

    /// The amounts of token_0 and token_1 that are owed to the protocol.
    pub protocol_fees_token_0: u64,
    pub protocol_fees_token_1: u64,

    /// The amounts in and out of swap token_0 and token_1
    pub swap_in_amount_token_0: u128,
    pub swap_out_amount_token_1: u128,
    pub swap_in_amount_token_1: u128,
    pub swap_out_amount_token_0: u128,
    /// Packed initialized tick array state
    pub tick_array_bitmap: [u64; 16],
    pub total_fees_token_0: u64,
    pub total_fees_token_1: u64,
    /// except protocol_fee and fund_fee
    pub total_fees_claimed_token_0: u64,
    pub total_fees_claimed_token_1: u64,

    pub fund_fees_token_0: u64,
    pub fund_fees_token_1: u64,
}

impl PoolStateDiff {
    fn from(pool_state: &PoolState) -> Self {
        Self {
            liquidity: pool_state.liquidity,
            sqrt_price_x64: pool_state.sqrt_price_x64,
            tick_current: pool_state.tick_current,
            fee_growth_global_0_x64: pool_state.fee_growth_global_0_x64,
            fee_growth_global_1_x64: pool_state.fee_growth_global_1_x64,
            protocol_fees_token_0: pool_state.protocol_fees_token_0,
            protocol_fees_token_1: pool_state.protocol_fees_token_1,
            swap_in_amount_token_0: pool_state.swap_in_amount_token_0,
            swap_out_amount_token_1: pool_state.swap_out_amount_token_1,
            swap_in_amount_token_1: pool_state.swap_in_amount_token_1,
            swap_out_amount_token_0: pool_state.swap_out_amount_token_0,
            tick_array_bitmap: pool_state.tick_array_bitmap,
            total_fees_token_0: pool_state.total_fees_token_0,
            total_fees_token_1: pool_state.total_fees_token_1,
            total_fees_claimed_token_0: pool_state.total_fees_claimed_token_0,
            total_fees_claimed_token_1: pool_state.total_fees_claimed_token_1,
            fund_fees_token_0: pool_state.fund_fees_token_0,
            fund_fees_token_1: pool_state.fund_fees_token_1,
        }
    }
}

#[derive(Debug, SerdeDiff, Serialize, Deserialize)]
struct TickArrayStateDiff {
    pub start_tick_index: i32,
    pub ticks: Vec<TickStateDiff>,
    pub initialized_tick_count: u8,
}

impl TickArrayStateDiff {
    fn from(tick_array_state: TickArrayState) -> Self {
        let diff: Vec<TickStateDiff> = tick_array_state
            .ticks
            .iter()
            .map(|tick| TickStateDiff::from(*tick))
            .collect();
        Self {
            start_tick_index: tick_array_state.start_tick_index,
            ticks: diff,
            initialized_tick_count: tick_array_state.initialized_tick_count,
        }
    }
}

#[derive(Debug, SerdeDiff, Serialize, Deserialize)]
struct TickStateDiff {
    pub tick: i32,
    /// Amount of net liquidity added (subtracted) when tick is crossed from left to right (right to left)
    pub liquidity_net: i128,
    /// The total position liquidity that references this tick
    pub liquidity_gross: u128,

    /// Fee growth per unit of liquidity on the _other_ side of this tick (relative to the current tick)
    /// only has relative meaning, not absolute — the value depends on when the tick is initialized
    pub fee_growth_outside_0_x64: u128,
    pub fee_growth_outside_1_x64: u128,
}

impl TickStateDiff {
    fn from(tick_sate: TickState) -> Self {
        Self {
            tick: tick_sate.tick,
            liquidity_net: tick_sate.liquidity_net,
            liquidity_gross: tick_sate.liquidity_gross,
            fee_growth_outside_0_x64: tick_sate.fee_growth_outside_0_x64,
            fee_growth_outside_1_x64: tick_sate.fee_growth_outside_1_x64,
        }
    }
}

#[derive(Debug, SerdeDiff, Serialize, Deserialize)]
struct TickArrayBitmapExtensionDiff {
    pub positive_tick_array_bitmap: [[u64; 8]; 14],
    /// Packed initialized tick array state for start_tick_index is negitive
    pub negative_tick_array_bitmap: [[u64; 8]; 14],
}

impl TickArrayBitmapExtensionDiff {
    fn from(bitmap_extension: TickArrayBitmapExtension) -> Self {
        Self {
            positive_tick_array_bitmap: bitmap_extension.positive_tick_array_bitmap,
            negative_tick_array_bitmap: bitmap_extension.negative_tick_array_bitmap,
        }
    }
}
