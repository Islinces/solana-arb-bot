use crate::arbitrage::jupiter_route::RouteBuilder;
use crate::arbitrage::types::route_plan_step::RoutePlanStep;
use crate::arbitrage::types::swap::Swap;
use crate::arbitrage::JUPITER_ID;
use crate::dex::{get_mint_program, DexQuoteResult};
use crate::interface::InstructionItem;
use anyhow::anyhow;
use async_trait::async_trait;
use base64::engine::general_purpose;
use base64::Engine;
use bincode::config;
use burberry::Executor;
use dashmap::{DashMap, DashSet};
use eyre::eyre;
use futures_util::future::{err, ok};
use rand::Rng;
use reqwest::{Client, Error, Response};
use serde::{Deserialize, Serialize};
use serde_json::json;
use solana_sdk::address_lookup_table::AddressLookupTableAccount;
use solana_sdk::compute_budget::ComputeBudgetInstruction;
use solana_sdk::hash::Hash;
use solana_sdk::instruction::{AccountMeta, Instruction};
use solana_sdk::message::v0::Message;
use solana_sdk::pubkey::Pubkey;
use solana_sdk::signature::Keypair;
use solana_sdk::signer::Signer;
use solana_sdk::transaction::VersionedTransaction;
use spl_associated_token_account::instruction::create_associated_token_account_idempotent;
use spl_associated_token_account::{get_associated_token_address_with_program_id, solana_program};
use spl_token::instruction::transfer;
use std::collections::HashMap;
use std::ops::{Div, Mul, Sub};
use std::str::FromStr;
use std::sync::Arc;
use std::time::{Duration, Instant};
use tokio::sync::{Mutex, RwLock};
use tracing::{error, info, instrument, warn};

const DEFAULT_TIP_ACCOUNTS: [&str; 8] = [
    "3AVi9Tg9Uo68tJfuvoKvqKNWKkC5wPdSSdeBnizKZ6jT",
    "96gYZGLnJYVFmbjzopPSU6QiEV5fGqZNyN9nmNhvrZU5",
    "ADaUMid9yfUytqMBgopwjb2DTLSokTSzL1zt6iGPaS49",
    "Cw8CFyM9FkoMi7K7Crf6HNQqf4uEMzpKw6QNghXLvLkY",
    "HFqU5x63VTqvQss8hp11i4wVV8bD44PvwucfZ2bU7gRe",
    "DfXygSm4jCyNCybVYYK6DwvWqjKee8pbDmJGcLWNDXjh",
    "ADuUkR4vqLUMWXxW9gh6D6L8pMSawimctcNZ5pGwDcEt",
    "DttWaMuVvTiduZRnguLF7jNxTgiMBZ1hyAumKUiL2KRL",
];

fn get_jito_fee_account_with_rand() -> Pubkey {
    let mut rng = rand::thread_rng();
    Pubkey::from_str(DEFAULT_TIP_ACCOUNTS[rng.gen_range(0..=7)]).unwrap()
}

pub struct JitoConfig {
    pub jito_region: String,
    pub jito_uuid: Option<String>,
}

#[derive(Deserialize)]
struct JitoResponse {
    result: String,
}

#[derive(Debug)]
pub struct JitoArbExecutor {
    cached_blockhash: Arc<RwLock<Hash>>,
    keypair: Keypair,
    bot_name: Option<String>,
    // mint --> mint ata
    mint_ata: Arc<DashMap<Pubkey, u64>>,
    native_ata: Pubkey,
    client: Arc<Client>,
    jito_url: String,
    tip_bps_numerator: u64,
    tip_bps_denominator: u64,
}

#[async_trait]
impl Executor<DexQuoteResult> for JitoArbExecutor {
    fn name(&self) -> &str {
        "SwapExecutor"
    }

    async fn execute(&self, quote_result: DexQuoteResult) -> eyre::Result<()> {
        let start_time = quote_result.start_time;
        let route_calculate_cost = quote_result.route_calculate_cost;
        let amount_in = quote_result.amount_in;
        let amount_in = amount_in;
        let grpc_cost = quote_result.grop_cost.unwrap_or(0);
        let calac_format = format!(
            "{} -> {} -> {}, profit : {}",
            amount_in, quote_result.first_amount_out, quote_result.amount_out, quote_result.profit
        );
        let latest_blockhash = {
            let guard = self.cached_blockhash.read().await;
            *guard
        };
        match self.create_jito_bundle(latest_blockhash, quote_result) {
            Ok((bundle, instruction_cost)) => {
                let jito_request_start = Instant::now();
                let bundles = bundle
                    .into_iter()
                    .map(|item| bincode::serialize(&item).unwrap())
                    .map(|byte| general_purpose::STANDARD.encode(&byte))
                    .collect::<Vec<_>>();
                info!("bundles : {:#?}", bundles);
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
                // info!("{}", data.to_string());
                let mut bundle_id;
                let jito_response = self
                    .client
                    .clone()
                    .post(self.jito_url.clone())
                    .header("Content-Type", "application/json")
                    .json(&data)
                    .send()
                    .await;
                match jito_response {
                    Ok(response) => {
                        match response.text().await {
                            Ok(result) => {
                                bundle_id = result;
                            }
                            Err(err) => {
                                error!("Jito error: {:#?}", err);
                                bundle_id = "".to_string();
                            }
                        }
                        // bundle_id = response.json::<String>().await?;
                        // info!("Jito successful, bundle : {}", bundle_id);
                    }
                    Err(e) => {
                        bundle_id = "".to_string();
                        error!("Jito error: {}", e);
                    }
                }
                let send_jito_request_cost = jito_request_start.elapsed();

                info!("耗时: {}ms, GRPC耗时: {}ns, 路由: {}ns, 指令: {}ns, 发送: {}ms, Size: {}, Hash: {:?}, BundleId: {}, 计算过程: {}",
                    start_time.unwrap().elapsed().as_millis(),
                    grpc_cost,
                    route_calculate_cost.unwrap(),
                    instruction_cost.as_nanos(),
                    send_jito_request_cost.as_millis(),
                    (amount_in as f64).div(10_i32.pow(9) as f64),
                    latest_blockhash.to_string().get(40..).unwrap(),
                    bundle_id,
                    calac_format
                );
                Ok(())
            }
            Err(e) => Err(eyre!("Jito生成bundle失败, {:?}", e)),
        }
    }
}

impl JitoArbExecutor {
    pub fn new(
        bot_name: Option<String>,
        keypair: Keypair,
        mint_ata: Arc<DashMap<Pubkey, u64>>,
        native_ata: Pubkey,
        cached_blockhash: Arc<RwLock<Hash>>,
        jito_config: JitoConfig,
        tip_bps_numerator: u64,
        tip_bps_denominator: u64,
    ) -> Self {
        let jito_host = if jito_config.jito_region == "mainnet".to_string() {
            "https://mainnet.block-engine.jito.wtf".to_string()
        } else {
            format!(
                "https://{}.mainnet.block-engine.jito.wtf",
                jito_config.jito_region
            )
        };
        let jito_url = if jito_config.jito_uuid.is_none() {
            format!("{}/api/v1/bundles", jito_host)
        } else {
            format!(
                "{}/api/v1/bundles?uuid={}",
                jito_host,
                jito_config.jito_uuid.unwrap()
            )
        };
        Self {
            bot_name,
            cached_blockhash,
            keypair,
            mint_ata,
            native_ata,
            //TODO: http配置
            client: Arc::new(Client::new()),
            jito_url,
            tip_bps_numerator,
            tip_bps_denominator,
        }
    }

    pub fn build_jupiter_swap_ix(
        &self,
        quote_result: DexQuoteResult,
        tip: u64,
    ) -> Option<(Instruction, Vec<AddressLookupTableAccount>)> {
        let amount_in_mint = quote_result.amount_in_mint;
        let wallet = self.keypair.pubkey();
        let mut route_builder = RouteBuilder::new();
        let mut remaining_accounts = Vec::with_capacity(100);
        let mut alts = Vec::with_capacity(2);
        let mut route_plan = Vec::with_capacity(quote_result.instruction_items.len());
        for (index, item) in quote_result.instruction_items.into_iter().enumerate() {
            let (swap, program_id) = item.get_swap_type();
            if let Some((accounts, item_alts)) = item.parse_account_meta(wallet) {
                remaining_accounts.push(AccountMeta::new_readonly(program_id, false));
                remaining_accounts.extend(accounts);
                if swap == Swap::MeteoraDlmm || swap == Swap::RaydiumClmm {
                    remaining_accounts.push(AccountMeta::new_readonly(JUPITER_ID, false));
                }
                alts.extend(item_alts);
                route_plan.push(RoutePlanStep {
                    swap,
                    percent: 100,
                    input_index: if index == 0 { 0 } else { 1 },
                    output_index: if index == 0 { 1 } else { 0 },
                })
            } else {
                return None;
            }
        }
        // info!(
        //     "remaining_accounts: {:#?}",
        //     remaining_accounts
        //         .iter()
        //         .map(|a| a.pubkey.to_string())
        //         .collect::<Vec<_>>()
        // );
        route_builder
            .user_transfer_authority(wallet)
            .user_source_token_account(self.native_ata)
            .user_destination_token_account(self.native_ata)
            .destination_mint(amount_in_mint)
            .program(crate::arbitrage::JUPITER_ID)
            .in_amount(quote_result.amount_in)
            .quoted_out_amount(quote_result.amount_in + tip + 10_000)
            .slippage_bps(0)
            .platform_fee_bps(0)
            .route_plan(route_plan)
            .add_remaining_accounts(remaining_accounts.as_slice());
        Some((route_builder.instruction(), alts))
    }

    pub fn create_jito_bundle(
        &self,
        latest_blockhash: Hash,
        dex_quote_result: DexQuoteResult,
    ) -> anyhow::Result<(Vec<VersionedTransaction>, Duration)> {
        let start = Instant::now();
        let wallet = self.keypair.pubkey();
        // ======================第一个Transaction====================
        // TODO: 使用参数tip_bps
        let tip = dex_quote_result
            .profit
            .mul(self.tip_bps_numerator)
            .div(self.tip_bps_denominator);

        let mut first_instructions = Vec::with_capacity(6);
        // 设置 CU
        first_instructions.push(ComputeBudgetInstruction::set_compute_unit_limit(
            self.calculate_compute_unit(),
        ));
        // 设置 swap
        let jupiter_swap_result = self.build_jupiter_swap_ix(dex_quote_result, tip);
        if jupiter_swap_result.is_none() {
            return Err(anyhow!("生成 Swap ix 失败"));
        }
        let (jupiter_swap_ix, alts) = jupiter_swap_result.unwrap();
        first_instructions.push(jupiter_swap_ix);
        // MEMO
        if let Some(name) = self.bot_name.as_ref() {
            first_instructions.push(Instruction::new_with_bytes(
                Pubkey::from_str("Memo1UhkJRfHyvLMcVucJwxXeuD728EqVDDwQDxFMNo")?,
                name.as_bytes(),
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
            &get_mint_program(),
        );
        // 生成ATA账户执行
        first_instructions.push(create_associated_token_account_idempotent(
            &wallet,
            &dst_wallet,
            &spl_token::native_mint::id(),
            &get_mint_program(),
        ));
        let source_ata = self.native_ata.clone();
        // 转移WSQL到ATA账户中
        first_instructions.push(transfer(
            &get_mint_program(),
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
            &[&self.keypair],
        )?;
        // ======================第二个Transaction====================
        let mut second_instructions = Vec::with_capacity(4);
        // 设置CU
        second_instructions.push(ComputeBudgetInstruction::set_compute_unit_limit(3_500));
        // 关闭WSOL账户，并将WSOL转换为SOL，最终转移到创建的临时钱包中
        second_instructions.push(spl_token::instruction::close_account(
            &get_mint_program(),
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

    fn calculate_compute_unit(&self) -> u32 {
        300_000
    }
}
