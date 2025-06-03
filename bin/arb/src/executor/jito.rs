use crate::arb_bot::Command;
use crate::executor::Executor;
use crate::interface::MINT_PROGRAM_ID;
use crate::metadata::{get_arb_mint_ata, get_keypair, get_last_blockhash, remove_already_ata};
use crate::quoter::QuoteResult;
use anyhow::anyhow;
use base64::engine::general_purpose;
use base64::Engine;
use rand::Rng;
use reqwest::Client;
use serde_json::json;
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
use std::str::FromStr;
use std::sync::Arc;
use std::time::Duration;
use tokio::time::Instant;
use tracing::info;

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

fn get_jito_fee_account_with_rand() -> Pubkey {
    DEFAULT_TIP_ACCOUNTS[rand::rng().random_range(0..=7)]
}

pub struct JitoExecutor {
    bot_name: Option<String>,
    jito_url: String,
    client: Arc<Client>,
    tip_bps_numerator: u64,
    tip_bps_denominator: u64,
}

#[async_trait::async_trait]
impl Executor for JitoExecutor {
    fn initialize(command: &Command) -> anyhow::Result<Arc<dyn Executor>>
    where
        Self: Sized,
    {
        let bot_name = command.arb_bot_name.clone();
        let jito_region = command.jito_region.clone();
        let jito_uuid = command.jito_uuid.clone();
        let tip_bps_numerator = command.tip_bps_numerator;
        let tip_bps_denominator = command.tip_bps_denominator;
        let jito_host = if jito_region == "mainnet".to_string() {
            "https://mainnet.block-engine.jito.wtf".to_string()
        } else {
            format!("https://{}.mainnet.block-engine.jito.wtf", jito_region)
        };
        let jito_url = if jito_uuid.is_none() {
            format!("{}/api/v1/bundles", jito_host)
        } else {
            format!("{}/api/v1/bundles?uuid={}", jito_host, jito_uuid.unwrap())
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

        Ok(Arc::new(Self {
            bot_name,
            jito_url,
            client,
            tip_bps_numerator,
            tip_bps_denominator,
        }))
    }

    async fn execute(
        &self,
        quote_result: QuoteResult,
        tx: String,
        slot: u64,
    ) -> anyhow::Result<String> {
        match self.create_jito_bundle(quote_result, tx, slot).await {
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
                let data = json!({
                    "id": 1,
                    "jsonrpc": "2.0",
                    "method":"sendBundle",
                    "params": params
                });
                let jito_response = self
                    .client
                    .post(self.jito_url.clone())
                    .header("Content-Type", "application/json")
                    .json(&data)
                    .send()
                    .await;

                let bundle_id = match jito_response {
                    Ok(response) => {
                        let v: serde_json::Value = response.json().await?;
                        if let Some(id) = v.get("result").and_then(|r| r.as_str()) {
                            id.to_owned()
                        } else if let Some(msg) = v
                            .get("error")
                            .and_then(|e| e.get("message"))
                            .and_then(|m| m.as_str())
                        {
                            format!("Jito returned error: {}", msg)
                        } else {
                            format!("Unknown response format: {}", v)
                        }
                    }
                    Err(e) => {
                        format!("Jito returned error: {}", e)
                    }
                };
                Ok(format!(
                    "指令 : {:>4.2}μs, 发送 : {:>4.2}ms, BundleId : {} \n\nBase64 : {}",
                    instruction_cost.as_nanos() as f64 / 1000.0,
                    jito_request_start.elapsed().as_micros() as f64 / 1000.0,
                    bundle_id,
                    bundles.first().unwrap_or(&"".to_string()),
                ))
            }
            Err(e) => Err(anyhow!("Jito生成bundle失败, {}", e)),
        }
    }
}

impl JitoExecutor {
    fn calculate_compute_unit() -> u32 {
        300_000
    }

    async fn create_jito_bundle(
        &self,
        quote_result: QuoteResult,
        tx: String,
        slot: u64,
    ) -> anyhow::Result<(Vec<VersionedTransaction>, Duration)> {
        let start = Instant::now();
        let keypair = get_keypair();
        let wallet = keypair.pubkey();
        // ======================第一个Transaction====================
        let tip = (quote_result.profit as u64)
            .mul(self.tip_bps_numerator)
            .div(self.tip_bps_denominator);

        let mut first_instructions = Vec::with_capacity(10);
        // 设置 CU
        first_instructions.push(ComputeBudgetInstruction::set_compute_unit_limit(
            Self::calculate_compute_unit(),
        ));
        // 设置 swap
        let mut used_atas = quote_result.hop_path.get_relate_mint_ata(&wallet);
        let jupiter_swap_result = crate::jupiter::build_jupiter_swap_ix(
            quote_result.to_instructions()?,
            quote_result.swaped_mint().unwrap(),
            quote_result.amount_in,
            tip,
        );
        if jupiter_swap_result.is_none() {
            return Err(anyhow!("生成 Swap ix 失败"));
        }
        let (jupiter_swap_ix, alts) = jupiter_swap_result.unwrap();
        remove_already_ata(&mut used_atas).await;
        for (_, mint) in used_atas {
            first_instructions.push(create_associated_token_account_idempotent(
                &wallet,
                &wallet,
                &mint,
                &MINT_PROGRAM_ID,
            ));
        }
        first_instructions.push(jupiter_swap_ix);
        // MEMO
        if let Some(name) = self.bot_name.as_ref() {
            let memo_name = format!("{}-{}-{}", name.as_str(), tx, slot);
            first_instructions.push(Instruction::new_with_bytes(
                Pubkey::from_str("Memo1UhkJRfHyvLMcVucJwxXeuD728EqVDDwQDxFMNo")?,
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
        let latest_blockhash = get_last_blockhash().await;
        let first_message = Message::try_compile(
            &wallet,
            &first_instructions,
            alts.as_slice(),
            latest_blockhash,
        )?;
        // info!("alt {:#?}", alts.as_slice());
        // info!("first_message {:#?}", first_message);
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
