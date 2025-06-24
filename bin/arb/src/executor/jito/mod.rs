use crate::arb_bot::Command;
use crate::dex::get_token_program;
use crate::dex::{MEMO_PROGRAM, MINT_PROGRAM_ID};
use crate::executor::Executor;
use crate::graph::SearchResult;
use crate::metadata::{get_arb_mint_ata, get_keypair, get_last_blockhash};
use crate::HopPathSearchResult;
use aes_gcm::aead::Aead;
use aes_gcm::{Aes256Gcm, Key, KeyInit, Nonce};
use anyhow::anyhow;
use anyhow::Result;
use base64::engine::general_purpose;
use base64::Engine;
use clap::ValueEnum;
use futures_util::future::err;
use parking_lot::RwLock;
use rand::Rng;
use rand_core::OsRng;
use rand_core::RngCore;
use rayon::prelude::IntoParallelRefIterator;
use reqwest::{Client, Error, Response};
use serde::Deserialize;
use serde_json::{json, Value};
use solana_sdk::compute_budget::ComputeBudgetInstruction;
use solana_sdk::instruction::Instruction;
use solana_sdk::message::v0::Message;
use solana_sdk::pubkey;
use solana_sdk::pubkey::Pubkey;
use solana_sdk::signature::{Keypair, Signer};
use solana_sdk::transaction::VersionedTransaction;
use spl_associated_token_account::instruction::create_associated_token_account_idempotent;
use spl_associated_token_account::{get_associated_token_address_with_program_id, solana_program};
use spl_token::instruction::transfer;
use std::ops::{Div, Mul};
use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::Arc;
use std::time::Duration;
use tokio::sync::OnceCell;
use tokio::time::Instant;
use tracing::{error, info};

const DEFAULT_TIP_ACCOUNTS: [Pubkey; 8] = [
    pubkey!("3AVi9Tg9Uo68tJfuvoKvqKNWKkC5wPdSSdeBnizKZ6jT"),
    pubkey!("96gYZGLnJYVFmbjzopPSU6QiEV5fGqZNyN9nmNhvrZU5"),
    pubkey!("ADaUMid9yfUytqMBgopwjb2DTLSokTSzL1zt6iGPaS49"),
    pubkey!("Cw8CFyM9FkoMi7K7Crf6HNQqf4uEMzpKw6QNghXLvLkY"),
    pubkey!("HFqU5x63VTqvQss8hp11i4wVV8bD44PvwucfZ2bU7gRe"),
    pubkey!("DfXygSm4jCyNCybVYYK6DwvWqjKee8pbDmJGcLWNDXjh"),
    pubkey!("ADuUkR4vqLUMWXxW9gh6D6L8pMSawimctcNZ5pGwDcEt"),
    pubkey!("DttWaMuVvTiduZRnguLF7jNxTgiMBZ1hyAumKUiL2KRL"),
];

static JITO_EMA_TIPS: OnceCell<RwLock<u64>> = OnceCell::const_new();

const JITO_UUID_KEYS: &[u8; 32] = b"mBE5O1xpYfaxfuFT6mdFBxhHnnkW39X8";

fn get_jito_fee_account_with_rand() -> Pubkey {
    DEFAULT_TIP_ACCOUNTS[rand::rng().random_range(0..=7)]
}

pub struct JitoExecutor {
    bot_name: Option<String>,
    jito_url: Vec<String>,
    used_url_index: Option<AtomicUsize>,
    client: Arc<Client>,
    tips_type: JitoTipsType,
}

#[derive(Debug, Deserialize)]
struct JitoTips {
    landed_tips_25th_percentile: f64,
    landed_tips_50th_percentile: f64,
    landed_tips_75th_percentile: f64,
    landed_tips_95th_percentile: f64,
    landed_tips_99th_percentile: f64,
    ema_landed_tips_50th_percentile: f64,
}

#[derive(Debug, Clone, Copy, ValueEnum)]
pub enum JitoTipsType {
    Th25,
    Th50,
    Th75,
    Th95,
    Th99,
    Ema,
}

async fn get_jito_ema_tips(tips_type: JitoTipsType) -> Result<u64> {
    let url = "https://bundles.jito.wtf/api/v1/bundles/tip_floor";
    let tips: Vec<JitoTips> = reqwest::get(url).await?.json().await?;
    match tips.first() {
        None => Err(anyhow!("无返回数据")),
        Some(tips) => {
            let percentile = match tips_type {
                JitoTipsType::Th25 => tips.landed_tips_25th_percentile,
                JitoTipsType::Th50 => tips.landed_tips_50th_percentile,
                JitoTipsType::Th75 => tips.landed_tips_75th_percentile,
                JitoTipsType::Th95 => tips.landed_tips_95th_percentile,
                JitoTipsType::Th99 => tips.landed_tips_99th_percentile,
                JitoTipsType::Ema => tips.ema_landed_tips_50th_percentile,
            };
            Ok((percentile * 1_000_000_000.0).floor() as u64)
        }
    }
}

#[async_trait::async_trait]
impl Executor for JitoExecutor {
    async fn initialize(command: &Command) -> Result<Arc<dyn Executor>>
    where
        Self: Sized,
    {
        let bot_name = command.arb_bot_name.clone();
        let jito_region = command.jito_region.clone();
        let jito_uuid = command.jito_uuid.clone();
        let tips_type = command.jito_tips_type.clone();
        let jito_host = if jito_region == "mainnet".to_string() {
            "https://mainnet.block-engine.jito.wtf".to_string()
        } else {
            format!("https://{}.mainnet.block-engine.jito.wtf", jito_region)
        };
        let jito_url = match jito_uuid {
            None => {
                vec![format!("{}/api/v1/bundles", jito_host)]
            }
            Some(uuid) => decrypt_base64(uuid.as_str(), JITO_UUID_KEYS)
                .expect("解析jito_uuid失败")
                .split(',')
                .map(String::from)
                .map(|id| format!("{}/api/v1/bundles?uuid={}", jito_host, id))
                .collect::<Vec<_>>(),
        };
        let client = Arc::new(
            Client::builder()
                .user_agent("jito-client/0.1")
                .http2_prior_knowledge() // BE 端启用了 h2，单链接多并发
                .pool_max_idle_per_host(32)
                .tcp_keepalive(Some(Duration::from_secs(2)))
                .http2_adaptive_window(true) // 动态窗口，吞吐更平滑
                .timeout(Duration::from_millis(250)) // 避免吊死的请求拖慢下一批
                .build()
                .expect("Failed to build HTTP client"),
        );
        JITO_EMA_TIPS.set(RwLock::new(get_jito_ema_tips(tips_type).await?))?;
        tokio::spawn(async move {
            loop {
                match get_jito_ema_tips(tips_type).await {
                    Ok(tips) => {
                        let mut write_guard = JITO_EMA_TIPS.get().unwrap().write();
                        *write_guard = tips;
                    }
                    Err(e) => {
                        error!("获取 jito_ema_tips 失败，{}", e);
                    }
                }
                tokio::time::sleep(Duration::from_secs(30)).await;
            }
        });
        let used_url_index = if jito_url.len() == 1 {
            None
        } else {
            Some(AtomicUsize::new(0))
        };
        Ok(Arc::new(Self {
            bot_name,
            jito_url,
            used_url_index,
            client,
            tips_type,
        }))
    }

    async fn execute(
        &self,
        hop_path_search_result: HopPathSearchResult,
        tx: String,
        slot: u64,
    ) -> Result<String> {
        match self
            .create_jito_bundle(hop_path_search_result, tx, slot)
            .await
        {
            Ok((bundle, instruction_cost)) => {
                let jito_request_start = Instant::now();
                let bundles = bundle
                    .into_iter()
                    .map(|item| bincode::serialize(&item).unwrap())
                    .map(|byte| general_purpose::STANDARD.encode(&byte))
                    .collect::<Vec<_>>();
                let transactions = json!(bundles);
                let params = json!([
                    transactions,
                    {
                        "encoding": "base64"
                    }
                ]);
                let data = Arc::new(json!({
                    "id": 1,
                    "jsonrpc": "2.0",
                    "method":"sendBundle",
                    "params": params
                }));
                match self.pick_jito_url() {
                    None => Err(anyhow!("获取JitoUrl失败")),
                    Some(jito_urls) => {
                        let jito_urls = jito_urls.clone();
                        jito_urls.into_iter().for_each(|jito_url| {
                            let cloned_data = data.clone();
                            let client = self.client.clone();
                            tokio::spawn(async move {
                                let _ = client
                                    .post(jito_url)
                                    .header("Content-Type", "application/json")
                                    .json(cloned_data.as_ref())
                                    .send()
                                    .await;
                            });
                        });
                        Ok(format!(
                            "指令 : {:>4.2}μs, 发送 : {:>4.2}ms, \nBase64 : {}",
                            instruction_cost.as_nanos() as f64 / 1000.0,
                            jito_request_start.elapsed().as_micros() as f64 / 1000.0,
                            bundles.first().unwrap_or(&"".to_string()),
                        ))
                    }
                }
            }
            Err(e) => Err(anyhow!("Jito生成bundle失败, {}", e)),
        }
    }
}

impl JitoExecutor {
    fn pick_jito_url(&self) -> Option<Vec<String>> {
        Some(self.jito_url.clone())
    }

    fn calculate_jito_tips(&self, _profit: i64) -> Result<u64> {
        Ok(JITO_EMA_TIPS
            .get()
            .ok_or(anyhow!("无法获取jito_ema_tips"))?
            .read()
            .clone())
    }

    fn calculate_compute_unit() -> u32 {
        250_000
    }

    async fn create_jito_bundle(
        &self,
        hop_path_search_result: HopPathSearchResult,
        tx: String,
        slot: u64,
    ) -> Result<(Vec<VersionedTransaction>, Duration)> {
        let start = Instant::now();
        let keypair = get_keypair();
        let wallet = keypair.pubkey();
        // ======================第一个Transaction====================
        let tip = self.calculate_jito_tips(hop_path_search_result.profit())?;

        let mut first_instructions = Vec::with_capacity(10);
        // 设置 CU
        first_instructions.push(ComputeBudgetInstruction::set_compute_unit_limit(
            Self::calculate_compute_unit(),
        ));
        // 设置 swap
        let (jupiter_swap_ix, uninitialized_atas, alts) =
            crate::jupiter::build_jupiter_swap_ix(hop_path_search_result, tip)?;
        for mint_ata_pair in uninitialized_atas {
            first_instructions.push(create_associated_token_account_idempotent(
                &wallet,
                &wallet,
                &mint_ata_pair.mint,
                &get_token_program(&mint_ata_pair.mint),
            ));
        }
        first_instructions.push(jupiter_swap_ix);
        // MEMO
        if let Some(name) = self.bot_name.as_ref() {
            let memo_name = format!("{}-{}-{}", name.as_str(), tx, slot);
            first_instructions.push(Instruction::new_with_bytes(
                MEMO_PROGRAM,
                memo_name.as_bytes(),
                vec![],
            ));
        }
        // 生成临时钱包
        let dst_keypair = Keypair::new();
        let dst_wallet = dst_keypair.pubkey();
        // 生成临时钱包WSOL ATA账户地址
        let dst_ata = get_associated_token_address_with_program_id(
            &dst_wallet,
            &spl_token::native_mint::id(),
            &MINT_PROGRAM_ID,
        );
        // 生成ATA账户执行
        first_instructions.push(create_associated_token_account_idempotent(
            &wallet,
            &dst_wallet,
            &spl_token::native_mint::id(),
            &MINT_PROGRAM_ID,
        ));
        let source_ata = get_arb_mint_ata();

        // 转移WSQL到ATA账户中
        first_instructions.push(transfer(
            &MINT_PROGRAM_ID,
            &source_ata,
            &dst_ata,
            &wallet,
            &[],
            tip + 10000,
        )?);
        // 转移SQL，用于支付账户租金+签名费
        first_instructions.push(solana_program::system_instruction::transfer(
            &wallet,
            &dst_wallet,
            2039280 + 5000,
        ));
        // 生成Transaction
        let latest_blockhash = get_last_blockhash();
        let first_message = Message::try_compile(
            &wallet,
            &first_instructions,
            alts.as_slice(),
            latest_blockhash,
        )?;
        let first_transaction = VersionedTransaction::try_new(
            solana_sdk::message::VersionedMessage::V0(first_message),
            &[keypair.as_ref()],
        )?;
        // ======================第二个Transaction====================
        let mut second_instructions = Vec::with_capacity(4);
        // 设置CU
        second_instructions.push(ComputeBudgetInstruction::set_compute_unit_limit(3_500));
        // 关闭WSOL账户，并将WSOL转换为SOL，最终转移到创建的临时钱包中
        second_instructions.push(spl_token::instruction::close_account(
            &MINT_PROGRAM_ID,
            &dst_ata,
            &dst_wallet,
            &dst_wallet,
            &[],
        )?);
        // 给JITO发小费
        second_instructions.push(solana_program::system_instruction::transfer(
            &dst_wallet,
            &get_jito_fee_account_with_rand(),
            tip,
        ));
        // 转移临时钱包SOL到主钱包
        second_instructions.push(solana_program::system_instruction::transfer(
            &dst_wallet,
            &wallet,
            2039280 + 2039280 + 10000,
        ));
        // 生成Transaction
        let second_message =
            Message::try_compile(&dst_wallet, &second_instructions, &[], latest_blockhash)?;
        let second_transaction = VersionedTransaction::try_new(
            solana_sdk::message::VersionedMessage::V0(second_message),
            &[&dst_keypair],
        )?;
        Ok((vec![first_transaction, second_transaction], start.elapsed()))
    }
}

/// 加密函数：输出 base64(nonce + ciphertext)
fn encrypt_base64(plaintext: &str, key_bytes: &[u8; 32]) -> String {
    let key = Key::<Aes256Gcm>::from_slice(key_bytes);
    let cipher = Aes256Gcm::new(key);

    // 随机 12 字节 nonce
    let mut nonce_bytes = [0u8; 12];
    OsRng.fill_bytes(&mut nonce_bytes);
    let nonce = Nonce::from_slice(&nonce_bytes);

    let ciphertext = cipher
        .encrypt(nonce, plaintext.as_bytes())
        .expect("encryption failure!");

    // 拼接 nonce + ciphertext
    let mut combined = nonce_bytes.to_vec();
    combined.extend(ciphertext);

    // base64 编码
    general_purpose::STANDARD.encode(combined)
}

/// 解密函数：输入 base64(nonce + ciphertext)，输出原文
fn decrypt_base64(encoded: &str, key_bytes: &[u8; 32]) -> anyhow::Result<String> {
    let combined = general_purpose::STANDARD
        .decode(encoded)
        .map_err(|e| anyhow!(format!("base64 decode error: {e}")))?;

    if combined.len() < 12 {
        return Err(anyhow!("输入的数据不合法"));
    }

    let (nonce_bytes, ciphertext) = combined.split_at(12);
    let nonce = Nonce::from_slice(nonce_bytes);
    let key = Key::<Aes256Gcm>::from_slice(key_bytes);
    let cipher = Aes256Gcm::new(key);

    let plaintext = cipher
        .decrypt(nonce, ciphertext)
        .map_err(|e| anyhow!(format!("decryption failed: {e}")))?;

    String::from_utf8(plaintext).map_or(Err(anyhow!("原因字符编码不是UTF-8")), |text| {
        Ok(text)
    })
}

mod test {
    use crate::executor::jito::{decrypt_base64, encrypt_base64, JITO_UUID_KEYS};
    use solana_sdk::pubkey::Pubkey;

    #[test]
    fn test_encrypt_base64() {
        let text = "1234,45678";
        let encrypt_text = encrypt_base64(text, JITO_UUID_KEYS);
        println!("密文: {:?}", encrypt_text);
        let origin_text = decrypt_base64(&encrypt_text, JITO_UUID_KEYS).unwrap();
        assert_eq!(text, origin_text);
        let vec = origin_text.split(',').map(String::from).collect::<Vec<_>>();
        println!("{:?}", vec);
    }
}
