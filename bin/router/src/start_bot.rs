use crate::arbitrage::arb_strategy::ArbStrategy;
use crate::arbitrage::jito_arb_executor::{JitoArbExecutor, JitoConfig};
use crate::arbitrage::message_collector::GrpcMessageCollector;
use crate::arbitrage::Action;
use crate::dex::get_mint_program;
use crate::interface::SourceMessage;
use crate::keypair::KeypairVault;
use burberry::{map_collector, map_executor, Engine};
use clap::Parser;
use dashmap::DashMap;
use rpassword::read_password;
use solana_client::nonblocking::rpc_client::RpcClient;
use solana_client::rpc_request::TokenAccountsFilter;
use solana_program::hash::Hash;
use solana_program::pubkey::Pubkey;
use solana_sdk::signature::Keypair;
use solana_sdk::signer::Signer;
use spl_associated_token_account::get_associated_token_address_with_program_id;
use std::borrow::Cow;
use std::collections::HashMap;
use std::env;
use std::ops::Mul;
use std::path::{Path, PathBuf};
use std::str::FromStr;
use std::sync::Arc;
use std::time::Duration;
use tokio::sync::Mutex;
use tracing::{error, info};

#[derive(Parser, Debug)]
pub struct Command {
    #[arg(long)]
    keypair_path: Option<String>,
    #[arg(long)]
    dex_json_path: Option<String>,
    #[arg(long, required = true)]
    amount_in: u64,
    #[arg(long)]
    bot_name: Option<String>,
    #[arg(long)]
    worker_size: Option<usize>,
    #[arg(long)]
    grpc_url: Option<String>,
    #[arg(long)]
    rpc_url: Option<String>,
    #[arg(long)]
    jito_region: Option<String>,
    #[arg(long)]
    jito_uuid: Option<String>,
    #[arg(long)]
    profit_threshold: Option<u64>,
}

pub enum ExecutorType {
    JITO(JitoConfig),
}

pub async fn run() -> anyhow::Result<()> {
    let command = Command::parse();
    let mut keypair = None;
    let keypair_path = command.keypair_path.unwrap_or("key.bin".to_string());
    loop {
        println!("请输入密码：");
        let input_password = read_password().expect("读取密码失败");
        let keypair_vault = KeypairVault::load(PathBuf::from_str(&keypair_path)?)?;
        if let Ok(k) = keypair_vault.decrypt(&input_password) {
            let wallet = &k.pubkey();
            keypair = Some(k);
            println!("密码正确，Pubkey : {:?}, 继续执行...", wallet);
            break;
        } else {
            println!("密码错误，请重新输入。");
        }
    }

    let keypair = keypair.unwrap();
    let dex_json_path = command.dex_json_path.unwrap_or("dex_data.json".to_string());
    let bot_name = command.bot_name;
    let arb_worker_size = command.worker_size.unwrap_or(20);
    let profit_threshold = command.profit_threshold.unwrap_or(100_000);
    let start_amount_in = command.amount_in;
    let wallet = keypair.pubkey();
    // RPC
    let grpc_url = command
        .grpc_url
        .unwrap_or("https://solana-yellowstone-grpc.publicnode.com".to_string());
    let rpc_url = command
        .rpc_url
        .unwrap_or("https://solana-rpc.publicnode.com".to_string());
    // JITO
    let jito_region = command
        .jito_region
        .unwrap_or_else(|| "frankfurt".to_string());
    let jito_uuid = command.jito_uuid;

    info!(
        "启动参数:\n \
    bot_name : {:?}\n \
    wallet :{wallet}\n \
    start_amount_in : {start_amount_in}\n \
    worker_size : {arb_worker_size}\n \
    profit_threshold : {profit_threshold}\n \
    grpc_url : {grpc_url}\n \
    rpc_url : {rpc_url}\n \
    jito_regin : {jito_region}\n \
    jito_uuid : {:?}\n",
        bot_name, jito_uuid
    );

    let rpc_client = Arc::new(RpcClient::new(rpc_url.to_string()));

    let native_mint_ata = get_associated_token_address_with_program_id(
        &wallet,
        &spl_token::native_mint::id(),
        &get_mint_program(),
    );
    let (wallet_all_ata_amount, native_mint_amount) =
        init_wallet_ata_account(rpc_client.clone(), wallet.clone(), native_mint_ata).await;
    let wallet_all_ata_amount = Arc::new(wallet_all_ata_amount);
    let native_mint_amount = Arc::new(Mutex::new(native_mint_amount));

    let initial_blockhash = rpc_client.get_latest_blockhash().await?;
    let cached_blockhash = Arc::new(Mutex::new(initial_blockhash));

    // 更新blockHash
    let refresh_interval = Duration::from_millis(500);
    let blockhash_client = rpc_client.clone();
    let blockhash_cache = cached_blockhash.clone();
    tokio::spawn(async move {
        blockhash_refresher(blockhash_client, blockhash_cache, refresh_interval).await;
    });
    // ata账户更新
    let wallet_all_ata_amount_cache = wallet_all_ata_amount.clone();
    let native_mint_amount_cache = native_mint_amount.clone();
    let wallet_ata_refresher_rpc_client = rpc_client.clone();
    tokio::spawn(async move {
        wallet_ata_refresher(
            wallet_ata_refresher_rpc_client.clone(),
            wallet,
            wallet_all_ata_amount_cache,
            native_mint_amount_cache,
            native_mint_ata,
            Duration::from_secs(60),
        )
        .await;
    });
    let executor_type = ExecutorType::JITO(JitoConfig {
        jito_region,
        jito_uuid,
    });
    let arb_executor = match executor_type {
        ExecutorType::JITO(jito_config) => JitoArbExecutor::new(
            bot_name,
            keypair,
            wallet_all_ata_amount.clone(),
            native_mint_ata,
            cached_blockhash.clone(),
            jito_config,
        ),
    };
    let mut engine = Engine::default();
    let message_collector =
        GrpcMessageCollector::new(rpc_client.clone(), grpc_url, dex_json_path.clone());
    engine.add_collector(map_collector!(message_collector, SourceMessage::Account));
    engine.add_executor(map_executor!(arb_executor, Action::SWAP));
    engine.add_strategy(Box::new(ArbStrategy::new(
        rpc_client.clone(),
        arb_worker_size,
        native_mint_amount,
        profit_threshold,
        None,
        None,
        dex_json_path,
        start_amount_in,
    )));
    engine.run_and_join().await.unwrap();
    Ok(())
}

async fn blockhash_refresher(
    rpc_client: Arc<RpcClient>,
    cached_blockhash: Arc<Mutex<Hash>>,
    refresh_interval: Duration,
) {
    loop {
        match rpc_client.get_latest_blockhash().await {
            Ok(blockhash) => {
                let mut guard = cached_blockhash.lock().await;
                *guard = blockhash;
            }
            Err(e) => {
                error!("Failed to refresh blockhash: {:?}", e);
            }
        }
        tokio::time::sleep(refresh_interval).await;
    }
}

async fn init_wallet_ata_account(
    rpc_client: Arc<RpcClient>,
    wallet: Pubkey,
    native_mint_ata: Pubkey,
) -> (DashMap<Pubkey, u64>, u64) {
    let wallet_all_ata_account = rpc_client
        .get_token_accounts_by_owner(&wallet, TokenAccountsFilter::ProgramId(get_mint_program()))
        .await
        .unwrap()
        .iter()
        .map(|a| (Pubkey::from_str(&a.pubkey).unwrap(), a.account.lamports))
        .collect::<DashMap<_, _>>();
    let native_mint_ata_amount = wallet_all_ata_account
        .get(&native_mint_ata)
        .unwrap()
        .clone();
    (wallet_all_ata_account, native_mint_ata_amount)
}

async fn wallet_ata_refresher(
    rpc_client: Arc<RpcClient>,
    wallet: Pubkey,
    wallet_ata_amount: Arc<DashMap<Pubkey, u64>>,
    wallet_native_ata_amount: Arc<Mutex<u64>>,
    native_mint_ata: Pubkey,
    refresh_interval: Duration,
) {
    loop {
        match rpc_client
            .get_token_accounts_by_owner(
                &wallet,
                TokenAccountsFilter::ProgramId(get_mint_program()),
            )
            .await
        {
            Ok(ata_accounts) => {
                let mut current_wallet_ata_amount = ata_accounts
                    .into_iter()
                    .map(|a| (Pubkey::from_str(&a.pubkey).unwrap(), a.account.lamports))
                    .collect::<HashMap<_, _>>();
                let mut guard = wallet_native_ata_amount.lock().await;
                let native_mint_ata_amount = current_wallet_ata_amount
                    .get(&native_mint_ata)
                    .unwrap()
                    .clone();
                *guard = native_mint_ata_amount;
                drop(guard);
                current_wallet_ata_amount.into_iter().for_each(|(a, b)| {
                    wallet_ata_amount.entry(a).or_insert(b);
                    wallet_ata_amount.entry(a).and_modify(|exist| *exist = b);
                });
            }
            Err(e) => {
                error!("Failed to refresh wallet_ata: {:?}", e);
            }
        }
        tokio::time::sleep(refresh_interval).await;
    }
}
