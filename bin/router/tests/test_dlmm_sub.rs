use arrayref::{array_ref, array_refs};
use base58::ToBase58;
use chrono::Local;
use log::{error, info};
use meteora_dlmm::sdk::commons::pda::derive_bin_array_bitmap_extension;
use meteora_dlmm::sdk::interface::accounts::{
    BinArray, BinArrayBitmapExtension, BinArrayBitmapExtensionAccount, LbPair,
};
use meteora_dlmm::sdk::interface::typedefs::{Bin, VariableParameters};
use solana_program::pubkey::Pubkey;
use std::collections::HashMap;
use std::env;
use std::io::Write;
use std::mem::offset_of;
use std::str::FromStr;
use std::time::Duration;
use yellowstone_grpc_client::{GeyserGrpcClient, Interceptor};
use yellowstone_grpc_proto::geyser::subscribe_update::UpdateOneof;
use yellowstone_grpc_proto::geyser::{
    CommitmentLevel, SubscribeRequest, SubscribeRequestAccountsDataSlice,
    SubscribeRequestFilterAccounts,
};
use yellowstone_grpc_proto::tonic::codegen::tokio_stream::StreamExt;

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
        // .subscribe_with_rexquest(Some(generate_pool_sub_field_request()))
        .subscribe_with_request(None)
        .await?;
    let (_, mut bin_array_stream) = client
        .as_mut()
        .unwrap()
        .subscribe_with_request(Some(generate_bin_array_sub_request()))
        // .subscribe_with_request(None)
        .await
        .unwrap();
    let (_, mut bin_array_bitmap_extension_stream) = client
        .as_mut()
        .unwrap()
        // .subscribe_with_request(Some(generate_bin_array_bitmap_sub_request()))
        .subscribe_with_request(None)
        .await?;
    info!("connect grpc successful!");
    loop {
        tokio::select! {
            update = stream.next() =>{
                if let Some(data) = update {
                    match data{
                       Ok(msg) => match msg.update_oneof {
                            Some(UpdateOneof::Account(account)) => {
                                let account_info = &account.account.unwrap();
                                let pubkey=&account_info.pubkey.to_base58();
                                let pool_state = pool_from_sub_data_slice(account_info.data.as_slice());
                                info!("txn : {:?}, account : {:?}, data:{:?}",
                                    &account_info.txn_signature.to_owned().unwrap().to_base58(),pubkey,pool_state);
                            },
                            _=>{}
                        },
                        _=>{}
                    }
                }
            },
            update = bin_array_stream.next() =>{
                if let Some(data) = update {
                    match data{
                       Ok(msg) => match msg.update_oneof {
                            Some(UpdateOneof::Account(account)) => {
                                let account_info = &account.account.unwrap();
                                let txn = &account_info.txn_signature;
                                let pubkey=&account_info.pubkey.to_base58();
                                // info!("txn : {:?},pubkey:{:?} data : {:?}",txn.to_owned().unwrap().to_base58(),pubkey,&account_info.data);
                                let  tick_array_state = bin_array_from_sub_data_slice(&account_info.data);
                                info!("txn : {:?}, bin_array_state_pubkey : {:?}, data:{:?}",txn.to_owned().unwrap().to_base58(),pubkey,tick_array_state);
                            },
                            _=>{}
                        },
                        _=>{}
                    }
                }
            },
            update = bin_array_bitmap_extension_stream.next() =>{
                if let Some(data) = update {
                    match data{
                       Ok(msg) => match msg.update_oneof {
                            Some(UpdateOneof::Account(account)) => {
                                let account_info = &account.account.unwrap();
                                let txn = &account_info.txn_signature;
                                let pubkey=&account_info.pubkey.to_base58();
                                let  tick_array_state = bin_array_bitmap_from_sub_data_slice(&account_info.data);
                                info!("txn : {:?}, bin_array_extension_bitmap_pubkey : {:?}, data:{:?}",txn.to_owned().unwrap().to_base58(),pubkey,tick_array_state);
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

fn generate_pool_sub_field_request() -> SubscribeRequest {
    let mut sub_accounts: HashMap<String, SubscribeRequestFilterAccounts> = HashMap::new();
    sub_accounts.insert(
        "5rCf1DM8LjKTw4YqhnoLcngyZYeNnQqztScTogYHAS6-account".to_string(),
        SubscribeRequestFilterAccounts {
            account: vec!["5rCf1DM8LjKTw4YqhnoLcngyZYeNnQqztScTogYHAS6".to_string()],
            ..Default::default()
        },
    );
    SubscribeRequest {
        accounts: sub_accounts,
        commitment: Some(CommitmentLevel::Processed).map(|x| x as i32),
        accounts_data_slice: vec![
            // v_parameters.volatility_accumulator
            SubscribeRequestAccountsDataSlice {
                offset: 40,
                length: 4,
            },
            // v_parameters.volatility_reference
            SubscribeRequestAccountsDataSlice {
                offset: 44,
                length: 4,
            },
            // v_parameters.index_reference
            SubscribeRequestAccountsDataSlice {
                offset: 48,
                length: 4,
            },
            // v_parameters.last_update_timestamp
            SubscribeRequestAccountsDataSlice {
                offset: 56,
                length: 8,
            },
            // pair_type
            SubscribeRequestAccountsDataSlice {
                offset: 75,
                length: 1,
            },
            // active_id
            SubscribeRequestAccountsDataSlice {
                offset: 76,
                length: 4,
            },
            // bin_step
            SubscribeRequestAccountsDataSlice {
                offset: 80,
                length: 2,
            },
            // status
            SubscribeRequestAccountsDataSlice {
                offset: 82,
                length: 1,
            },
            // activation_type
            SubscribeRequestAccountsDataSlice {
                offset: 86,
                length: 1,
            },
            // bin_array_bitmap
            SubscribeRequestAccountsDataSlice {
                offset: 584,
                length: 128,
            },
            // activation_point
            SubscribeRequestAccountsDataSlice {
                offset: 816,
                length: 8,
            },
        ],
        ..Default::default()
    }
}

fn pool_from_sub_data_slice(data: &[u8]) -> LbPair {
    let src = array_ref![data, 0, 165];
    let (
        v_p_volatility_accumulator,
        v_p_volatility_reference,
        v_p_index_reference,
        v_p_last_update_timestamp,
        pair_type,
        active_id,
        bin_step,
        status,
        activation_type,
        bin_array_bitmap,
        activation_point,
    ) = array_refs![src, 4, 4, 4, 8, 1, 4, 2, 1, 1, 128, 8];
    LbPair {
        v_parameters: VariableParameters {
            volatility_accumulator: u32::from_le_bytes(*v_p_volatility_accumulator),
            volatility_reference: u32::from_le_bytes(*v_p_volatility_reference),
            index_reference: i32::from_le_bytes(*v_p_index_reference),
            last_update_timestamp: i64::from_le_bytes(*v_p_last_update_timestamp),
            ..Default::default()
        },
        pair_type: u8::from_le_bytes(*pair_type),
        active_id: i32::from_le_bytes(*active_id),
        bin_step: u16::from_le_bytes(*bin_step),
        status: u8::from_le_bytes(*status),
        activation_type: u8::from_le_bytes(*activation_type),
        bin_array_bitmap: bin_array_bitmap
            .chunks_exact(8)
            .map(|chunk| u64::from_le_bytes(chunk.try_into().unwrap()))
            .collect::<Vec<_>>()
            .try_into()
            .unwrap(),
        activation_point: u64::from_le_bytes(*activation_point),
        ..Default::default()
    }
}

fn generate_bin_array_bitmap_sub_request() -> SubscribeRequest {
    let mut sub_accounts: HashMap<String, SubscribeRequestFilterAccounts> = HashMap::new();
    let bitmap_extension_pubkey = derive_bin_array_bitmap_extension(
        Pubkey::from_str("5rCf1DM8LjKTw4YqhnoLcngyZYeNnQqztScTogYHAS6").unwrap(),
    )
    .0;
    sub_accounts.insert(
        bitmap_extension_pubkey.to_string(),
        SubscribeRequestFilterAccounts {
            account: vec![bitmap_extension_pubkey.to_string()],
            ..Default::default()
        },
    );
    SubscribeRequest {
        accounts: sub_accounts,
        commitment: Some(CommitmentLevel::Processed).map(|x| x as i32),
        ..Default::default()
    }
}

fn bin_array_bitmap_from_sub_data_slice(data: &[u8]) -> BinArrayBitmapExtension {
    BinArrayBitmapExtensionAccount::deserialize(data).unwrap().0
}

fn generate_bin_array_sub_request() -> SubscribeRequest {
    let mut sub_bin_array_accounts: HashMap<String, SubscribeRequestFilterAccounts> =
        HashMap::new();
    sub_bin_array_accounts.insert(
        "5rCf1DM8LjKTw4YqhnoLcngyZYeNnQqztScTogYHAS6-account".to_string(),
        SubscribeRequestFilterAccounts {
            account: vec![
                "Etmp8eWWzzPqDWsxtCPXBarRzKHUCGTakxdCvD6PpnjX".to_string(),
                "boqZNMHK6BMzjXXWG69p7TSrPQ9QuAg1hALjP8nHRgH".to_string(),
                "BNtK1ZuAeLVezAVz8eZ5QXmhB74TtPBMYF1bkCf8EWh3".to_string(),
                "4UEN4a3BJnftS851rWuePeYQfGKht9skPL8CZLbUGx4n".to_string(),
            ],
            ..Default::default()
        },
    );
    let mut filter = Vec::new();
    // index
    filter.push(SubscribeRequestAccountsDataSlice {
        offset: 8,
        length: 8,
    });
    // version
    filter.push(SubscribeRequestAccountsDataSlice {
        offset: 16,
        length: 1,
    });
    // lb_pair
    filter.push(SubscribeRequestAccountsDataSlice {
        offset: 24,
        length: 32,
    });
    let mut start_index = 24 + 32;
    // bins 70个
    for _ in 0..70 {
        // amount_x
        filter.push(SubscribeRequestAccountsDataSlice {
            offset: start_index,
            length: 8,
        });
        start_index += 8;
        // amount_y
        filter.push(SubscribeRequestAccountsDataSlice {
            offset: start_index,
            length: 8,
        });
        start_index += 8;
        // price
        filter.push(SubscribeRequestAccountsDataSlice {
            offset: start_index,
            length: 16,
        });
        start_index += 16;
        start_index += (size_of::<Bin>() - 8 - 8 - 16) as u64
    }
    SubscribeRequest {
        accounts: sub_bin_array_accounts,
        commitment: Some(CommitmentLevel::Processed).map(|x| x as i32),
        accounts_data_slice: filter,
        ..Default::default()
    }
}

fn bin_array_from_sub_data_slice(data: &Vec<u8>) -> BinArray {
    let src = array_ref![data.as_slice(), 0, 2281];
    let (index, version, lb_pair, bins) = array_refs![src, 8, 1, 32, 2240];
    BinArray {
        index: i64::from_le_bytes(*index),
        version: u8::from_le_bytes(*version),
        lb_pair: Pubkey::from(lb_pair.to_owned()),
        bins: bins
            .chunks_exact(32)
            .map(|chunk| {
                let src = array_ref![chunk, 0, 32];
                let (amount_x, amount_y, price) = array_refs![src, 8, 8, 16];
                Bin {
                    amount_x: u64::from_le_bytes(*amount_x),
                    amount_y: u64::from_le_bytes(*amount_y),
                    price: u128::from_le_bytes(*price),
                    ..Default::default()
                }
            })
            .collect::<Vec<_>>()
            .try_into()
            .unwrap(),
        padding: [0; 7],
    }
}

#[test]
fn calac_lb_pair_sub_field_offset() {
    // 计算每个字段的offset和size
    let offsets = [
        (
            "v_parameters.volatility_accumulator",
            offset_of!(LbPair, v_parameters.volatility_accumulator) as usize,
            size_of::<u32>(),
        ),
        (
            "v_parameters.volatility_reference",
            offset_of!(LbPair, v_parameters.volatility_reference) as usize,
            size_of::<u32>(),
        ),
        (
            "v_parameters.index_reference",
            offset_of!(LbPair, v_parameters.index_reference) as usize,
            size_of::<i32>(),
        ),
        (
            "v_parameters.last_update_timestamp",
            offset_of!(LbPair, v_parameters.last_update_timestamp) as usize,
            size_of::<i64>(),
        ),
        (
            "pair_type",
            offset_of!(LbPair, pair_type) as usize,
            size_of::<u8>(),
        ),
        (
            "active_id",
            offset_of!(LbPair, active_id) as usize,
            size_of::<i32>(),
        ),
        (
            "bin_step",
            offset_of!(LbPair, bin_step) as usize,
            size_of::<u16>(),
        ),
        (
            "status",
            offset_of!(LbPair, status) as usize,
            size_of::<u8>(),
        ),
        (
            "activation_type",
            offset_of!(LbPair, activation_type) as usize,
            size_of::<u8>(),
        ),
        (
            "bin_array_bitmap",
            offset_of!(LbPair, bin_array_bitmap) as usize,
            size_of::<[u64; 16]>(),
        ),
        (
            "activation_point",
            offset_of!(LbPair, activation_point) as usize,
            size_of::<u64>(),
        ),
    ];
    let mut all_size = 0;
    // 打印结果
    for (field, offset, size) in offsets {
        all_size += size;
        println!(
            "Pool Field: {}, Offset: {} bytes, Size: {} bytes",
            field,
            offset + 8,
            size
        );
    }
    println!("all size: {}", all_size);
}

#[test]
fn calac_bin_array_sub_field_offset() {
    // 计算每个字段的offset和size
    let offsets = vec![
        (
            "index",
            offset_of!(BinArray, index) as usize,
            size_of::<i64>(),
        ),
        (
            "version",
            offset_of!(BinArray, version) as usize,
            size_of::<u8>(),
        ),
        (
            "lb_pair",
            offset_of!(BinArray, lb_pair) as usize,
            size_of::<Pubkey>(),
        ),
    ];
    let mut all_size = 0;
    for (field, offset, size) in offsets {
        all_size += size;
        println!(
            "BinArray \nField: {}, Offset: {} bytes, Size: {} bytes",
            field,
            offset + 8,
            size
        );
    }
    println!("bin array used all size: {}", all_size);
    //TickState
    let offsets = [
        (
            "amount_x",
            offset_of!(Bin, amount_x) as usize,
            size_of::<u64>(),
        ),
        (
            "amount_y",
            offset_of!(Bin, amount_y) as usize,
            size_of::<u64>(),
        ),
        ("price", offset_of!(Bin, price) as usize, size_of::<u128>()),
    ];
    all_size = 0;
    // 打印结果
    for (field, offset, size) in offsets {
        all_size += size;
        println!(
            "Bin \nField: {}, Offset: {} bytes, Size: {} bytes",
            field, offset, size
        );
    }
    println!("bin used all size: {}", all_size);
    println!("bin unused all size: {}", size_of::<Bin>() - all_size);
}
