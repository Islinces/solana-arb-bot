use crate::account_cache::get_token_program;
use crate::dex_data::DexJson;
use crate::interface::{ATA_PROGRAM_ID, MINT_PROGRAM_ID};
use crate::keypair::KeypairVault;
use ahash::{AHashMap, AHashSet};
use futures_util::future::join_all;
use parking_lot::RwLock;
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
use tokio::sync::OnceCell;
use tracing::{error, info};

static KEYPAIR: OnceCell<Arc<Keypair>> = OnceCell::const_new();
static WALLET_OF_ATA_AMOUNT: OnceCell<Arc<RwLock<AHashMap<Pubkey, u64>>>> = OnceCell::const_new();
static ARB_MINT_ATA_ACCOUNT: OnceCell<Pubkey> = OnceCell::const_new();
static LAST_BLOCK_HASH: OnceCell<Arc<RwLock<Hash>>> = OnceCell::const_new();

pub(crate) async fn init_metadata(
    keypair: Keypair,
    arb_mint: &Pubkey,
    dex_data: &[DexJson],
    rpc_client: Arc<RpcClient>,
) -> anyhow::Result<()> {
    KEYPAIR.set(Arc::new(keypair))?;
    let wallet = KEYPAIR.get().unwrap().pubkey();
    let arb_mint_ata =
        get_associated_token_address_with_program_id(&wallet, arb_mint, &MINT_PROGRAM_ID);
    ARB_MINT_ATA_ACCOUNT.set(arb_mint_ata)?;
    let mint_atas = dex_data
        .iter()
        .map(|json| vec![json.mint_a.clone(), json.mint_b.clone()])
        .flatten()
        .map(|mint| {
            Pubkey::find_program_address(
                &[
                    wallet.as_ref(),
                    get_token_program(&mint).as_ref(),
                    mint.as_ref(),
                ],
                &ATA_PROGRAM_ID,
            )
            .0
        })
        .collect::<AHashSet<_>>()
        .into_iter()
        .collect::<Vec<_>>();
    // ata账户更新
    let wallet_all_ata_amount = Arc::new(RwLock::new(
        init_wallet_ata_account(rpc_client.clone(), mint_atas.as_slice()).await,
    ));
    let wallet_all_ata_amount_cache = wallet_all_ata_amount.clone();
    WALLET_OF_ATA_AMOUNT.set(wallet_all_ata_amount)?;
    let ata_refresher_rpc_client = rpc_client.clone();
    tokio::spawn(async move {
        wallet_ata_refresher(
            ata_refresher_rpc_client,
            mint_atas,
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
                let mut guard = cached_blockhash.write();
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
    mint_atas: &[Pubkey],
) -> AHashMap<Pubkey, u64> {
    let ata_fut = mint_atas.iter().filter_map(|ata| async {
        match rpc_client.get_account(ata).await {
            Ok(account) => Some((ata.clone(), account)),
            Err(_) => None,
        }
    });
    let ata_accounts = join_all(ata_fut).await;
    let ata_accounts = ata_accounts
        .into_iter()
        .map(|(key, account)| (key, account.lamports))
        .collect::<AHashMap<_, _>>();
    info!(
        "钱包拥有的ATA账户 : {:?}",
        ata_accounts.iter().map(|(key, _)| key).collect::<Vec<_>>()
    );
    ata_accounts
}

async fn wallet_ata_refresher(
    rpc_client: Arc<RpcClient>,
    mint_atas: Vec<Pubkey>,
    wallet_ata_amount: Arc<RwLock<AHashMap<Pubkey, u64>>>,
    refresh_interval: Duration,
) {
    loop {
        {
            let wallet_ata_accounts =
                init_wallet_ata_account(rpc_client.clone(), mint_atas.as_slice()).await;
            let mut write_guard = wallet_ata_amount.write();
            *write_guard = wallet_ata_accounts;
        }
        tokio::time::sleep(refresh_interval).await;
    }
}

pub fn get_keypair() -> Arc<Keypair> {
    KEYPAIR.get().unwrap().clone()
}

pub async fn remove_already_ata(instruction_atas: &mut Vec<(Pubkey, Pubkey)>) {
    let read_guard = WALLET_OF_ATA_AMOUNT.get().unwrap().read();
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
        .get(&get_arb_mint_ata())
        .cloned()
}

pub async fn get_last_blockhash() -> Hash {
    LAST_BLOCK_HASH.get().unwrap().read().clone()
}
