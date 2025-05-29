use crate::interface::MINT_PROGRAM_ID;
use crate::keypair::KeypairVault;
use ahash::AHashMap;
use rpassword::read_password;
use solana_rpc_client::nonblocking::rpc_client::RpcClient;
use solana_rpc_client_api::request::TokenAccountsFilter;
use solana_sdk::hash::Hash;
use solana_sdk::pubkey::Pubkey;
use solana_sdk::signature::Keypair;
use solana_sdk::signer::Signer;
use spl_associated_token_account::get_associated_token_address_with_program_id;
use std::collections::HashMap;
use std::path::PathBuf;
use std::str::FromStr;
use std::sync::Arc;
use std::time::Duration;
use tokio::sync::{OnceCell, RwLock};
use tracing::error;

static KEYPAIR: OnceCell<Arc<Keypair>> = OnceCell::const_new();
static WALLET_OF_ATA_AMOUNT: OnceCell<Arc<RwLock<AHashMap<Pubkey, u64>>>> = OnceCell::const_new();
static ARB_MINT_ATA_ACCOUNT: OnceCell<Pubkey> = OnceCell::const_new();
static LAST_BLOCK_HASH: OnceCell<Arc<RwLock<Hash>>> = OnceCell::const_new();

pub(crate) async fn init_metadata(
    keypair_path: String,
    arb_mint: &Pubkey,
    rpc_client: Arc<RpcClient>,
) -> anyhow::Result<()> {
    loop {
        println!("请输入密码：");
        let input_password = "test_password123".to_string();
        let keypair_vault = KeypairVault::load(PathBuf::from_str(&keypair_path)?)?;
        if let Ok(k) = keypair_vault.decrypt(&input_password) {
            let wallet = &k.pubkey();
            println!("密码正确，Pubkey : {:?}, 继续执行...", wallet);
            KEYPAIR.set(Arc::new(k))?;
            break;
        } else {
            println!("密码错误，请重新输入。");
        }
    }
    let wallet = KEYPAIR.get().unwrap().pubkey();
    let arb_mint_ata =
        get_associated_token_address_with_program_id(&wallet, arb_mint, &MINT_PROGRAM_ID);
    ARB_MINT_ATA_ACCOUNT.set(arb_mint_ata)?;
    // ata账户更新
    let wallet_all_ata_amount = init_wallet_ata_account(rpc_client.clone(), wallet.clone()).await;
    let wallet_all_ata_amount = Arc::new(RwLock::new(wallet_all_ata_amount));
    let wallet_all_ata_amount_cache = wallet_all_ata_amount.clone();
    WALLET_OF_ATA_AMOUNT.set(wallet_all_ata_amount)?;
    let ata_refresher_rpc_client = rpc_client.clone();
    tokio::spawn(async move {
        wallet_ata_refresher(
            ata_refresher_rpc_client,
            wallet,
            wallet_all_ata_amount_cache,
            Duration::from_secs(60),
        )
        .await;
    });

    // 更新blockHash
    let initial_blockhash = rpc_client.clone().get_latest_blockhash().await?;
    let cached_blockhash = Arc::new(RwLock::new(initial_blockhash));
    let blockhash_cache = cached_blockhash.clone();
    LAST_BLOCK_HASH.set(cached_blockhash)?;
    let refresh_interval = Duration::from_millis(500);
    let blockhash_refresh_rpc_client = rpc_client.clone();
    tokio::spawn(async move {
        blockhash_refresher(
            blockhash_refresh_rpc_client,
            blockhash_cache,
            refresh_interval,
        )
        .await;
    });
    Ok(())
}

async fn blockhash_refresher(
    rpc_client: Arc<RpcClient>,
    cached_blockhash: Arc<RwLock<Hash>>,
    refresh_interval: Duration,
) {
    loop {
        match rpc_client.get_latest_blockhash().await {
            Ok(block_hash) => {
                let mut guard = cached_blockhash.write().await;
                *guard = block_hash;
            }
            Err(e) => {
                error!("BlockHash 刷新失败，原因: {}", e);
            }
        }
        tokio::time::sleep(refresh_interval).await;
    }
}

async fn init_wallet_ata_account(
    rpc_client: Arc<RpcClient>,
    wallet: Pubkey,
) -> AHashMap<Pubkey, u64> {
    rpc_client
        .get_token_accounts_by_owner(&wallet, TokenAccountsFilter::ProgramId(MINT_PROGRAM_ID))
        .await
        .unwrap()
        .iter()
        .map(|a| (Pubkey::from_str(&a.pubkey).unwrap(), a.account.lamports))
        .collect::<AHashMap<_, _>>()
}

async fn wallet_ata_refresher(
    rpc_client: Arc<RpcClient>,
    wallet: Pubkey,
    wallet_ata_amount: Arc<RwLock<AHashMap<Pubkey, u64>>>,
    refresh_interval: Duration,
) {
    loop {
        match rpc_client
            .get_token_accounts_by_owner(&wallet, TokenAccountsFilter::ProgramId(MINT_PROGRAM_ID))
            .await
        {
            Ok(ata_accounts) => {
                let current_wallet_ata_amount = ata_accounts
                    .into_iter()
                    .map(|a| (Pubkey::from_str(&a.pubkey).unwrap(), a.account.lamports))
                    .collect::<HashMap<_, _>>();
                let mut guard = wallet_ata_amount.write().await;
                current_wallet_ata_amount.into_iter().for_each(|(a, b)| {
                    guard.insert(a, b);
                });
            }
            Err(e) => {
                error!("Failed to refresh wallet_ata: {:?}", e);
            }
        }
        tokio::time::sleep(refresh_interval).await;
    }
}

pub fn get_keypair() -> Arc<Keypair> {
    KEYPAIR.get().unwrap().clone()
}

pub async fn remove_already_ata(instruction_atas: &mut Vec<(Pubkey, Pubkey)>) {
    let read_guard = WALLET_OF_ATA_AMOUNT.get().unwrap().read().await;
    instruction_atas.retain(|(ata, _)| !read_guard.contains_key(ata));
}

pub fn get_arb_mint_ata() -> Pubkey {
    ARB_MINT_ATA_ACCOUNT.get().unwrap().clone()
}

pub async fn get_arb_mint_ata_amount() -> Option<u64> {
    WALLET_OF_ATA_AMOUNT
        .get()
        .unwrap()
        .read()
        .await
        .get(&get_arb_mint_ata())
        .cloned()
}

pub async fn get_last_blockhash() -> Hash {
    LAST_BLOCK_HASH.get().unwrap().read().await.clone()
}
