use crate::arbitrage::types::swap::Swap;
use crate::arbitrage::types::swap::Swap::{Raydium, RaydiumClmm};
use crate::cache::Pool;
use crate::dex::meteora_dlmm::meteora_dlmm::{
    MeteoraDLMMCacheUpdater, MeteoraDLMMDex, MeteoraDLMMGrpcMessageOperator,
    MeteoraDLMMGrpcSubscribeRequestGenerator, MeteoraDLMMSnapshotFetcher,
};
use crate::dex::meteora_dlmm::pool_state::MeteoraDLMMInstructionItem;
use crate::dex::pump_fun::pool_state::PumpFunInstructionItem;
use crate::dex::pump_fun::pump_fun::{
    PumpFunAccountSnapshotFetcher, PumpFunCacheUpdater, PumpFunDex,
    PumpFunGrpcSubscribeRequestGenerator, PumpFunReadyGrpcMessageOperator,
};
use crate::dex::raydium_amm::pool_state::RaydiumAMMInstructionItem;
use crate::dex::raydium_amm::raydium_amm::{
    RaydiumAmmCacheUpdater, RaydiumAmmDex, RaydiumAmmGrpcMessageOperator,
    RaydiumAmmSnapshotFetcher, RaydiumAmmSubscribeRequestCreator,
};
use crate::dex::raydium_clmm::pool_state::RaydiumCLMMInstructionItem;
use crate::dex::raydium_clmm::raydium_clmm::{
    RaydiumCLMMCacheUpdater, RaydiumCLMMDex, RaydiumCLMMGrpcMessageOperator,
    RaydiumCLMMSnapshotFetcher, RaydiumCLMMSubscribeRequestCreator,
};
use crate::file_db::DexJson;
use crate::interface::SourceMessage::Account;
use anyhow::{anyhow, Result};
use serde::{Deserialize, Serialize};
use solana_client::nonblocking::rpc_client::RpcClient;
use solana_program::address_lookup_table::state::AddressLookupTable;
use solana_program::address_lookup_table::AddressLookupTableAccount;
use solana_program::clock::Clock;
use solana_program::instruction::AccountMeta;
use solana_program::program_pack::Pack;
use solana_program::pubkey::Pubkey;
use std::collections::HashMap;
use std::fmt::{Debug, Display, Formatter};
use std::sync::Arc;
use tokio::io::join;
use tokio::task::JoinSet;
use tracing::{error, instrument};
use yellowstone_grpc_proto::geyser::{
    CommitmentLevel, SubscribeRequest, SubscribeRequestAccountsDataSlice,
    SubscribeRequestFilterAccounts, SubscribeUpdateAccount,
};

pub type SubscribeKey = (DexType, GrpcAccountUpdateType);

#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub enum DexType {
    RaydiumAMM,
    RaydiumCLmm,
    PumpFunAMM,
    MeteoraDLMM,
}

impl Display for DexType {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        f.write_str(match self {
            DexType::RaydiumAMM => "RaydiumAMM",
            DexType::RaydiumCLmm => "RaydiumCLmm",
            DexType::PumpFunAMM => "PumpFunAM",
            DexType::MeteoraDLMM => "MeteoraDLMM",
        })
    }
}

impl From<Pubkey> for DexType {
    fn from(value: Pubkey) -> Self {
        if value == crate::dex::raydium_amm::ID {
            DexType::RaydiumAMM
        } else if value == crate::dex::raydium_clmm::ID {
            DexType::RaydiumCLmm
        } else if value == crate::dex::pump_fun::ID {
            DexType::PumpFunAMM
        } else if value == crate::dex::meteora_dlmm::ID {
            DexType::MeteoraDLMM
        } else {
            unreachable!()
        }
    }
}

impl DexType {
    pub fn get_owner(&self) -> Pubkey {
        match self {
            DexType::RaydiumAMM => crate::dex::raydium_amm::ID,
            DexType::RaydiumCLmm => crate::dex::raydium_clmm::ID,
            DexType::PumpFunAMM => crate::dex::pump_fun::ID,
            DexType::MeteoraDLMM => crate::dex::meteora_dlmm::ID,
        }
    }

    pub fn get_program_id(&self) -> Pubkey {
        self.get_owner()
    }

    pub fn get_subscribe_request_generator(
        &self,
    ) -> Result<Box<dyn GrpcSubscribeRequestGenerator>> {
        match self {
            DexType::RaydiumAMM => Ok(Box::new(RaydiumAmmSubscribeRequestCreator)),
            DexType::RaydiumCLmm => Ok(Box::new(RaydiumCLMMSubscribeRequestCreator)),
            DexType::PumpFunAMM => Ok(Box::new(PumpFunGrpcSubscribeRequestGenerator)),
            DexType::MeteoraDLMM => Ok(Box::new(MeteoraDLMMGrpcSubscribeRequestGenerator)),
        }
    }

    pub fn get_grpc_message_operator(
        &self,
        account_update: AccountUpdate,
    ) -> Result<Box<dyn ReadyGrpcMessageOperator>> {
        match self {
            DexType::RaydiumAMM => Ok(Box::new(RaydiumAmmGrpcMessageOperator::new(account_update))),
            DexType::RaydiumCLmm => Ok(Box::new(RaydiumCLMMGrpcMessageOperator::new(
                account_update,
            ))),
            DexType::PumpFunAMM => Ok(Box::new(PumpFunReadyGrpcMessageOperator::new(
                account_update,
            ))),
            DexType::MeteoraDLMM => Ok(Box::new(MeteoraDLMMGrpcMessageOperator::new(
                account_update,
            ))),
        }
    }

    pub fn get_snapshot_fetcher(&self) -> Result<Box<dyn AccountSnapshotFetcher>> {
        match self {
            DexType::RaydiumAMM => Ok(Box::new(RaydiumAmmSnapshotFetcher)),
            DexType::RaydiumCLmm => Ok(Box::new(RaydiumCLMMSnapshotFetcher)),
            DexType::PumpFunAMM => Ok(Box::new(PumpFunAccountSnapshotFetcher)),
            DexType::MeteoraDLMM => Ok(Box::new(MeteoraDLMMSnapshotFetcher)),
        }
    }

    pub fn get_cache_updater(&self, grpc_message: GrpcMessage) -> Result<Box<dyn CacheUpdater>> {
        match self {
            DexType::RaydiumAMM => Ok(Box::new(RaydiumAmmCacheUpdater::new(grpc_message)?)),
            DexType::RaydiumCLmm => Ok(Box::new(RaydiumCLMMCacheUpdater::new(grpc_message)?)),
            DexType::PumpFunAMM => Ok(Box::new(PumpFunCacheUpdater::new(grpc_message)?)),
            DexType::MeteoraDLMM => Ok(Box::new(MeteoraDLMMCacheUpdater::new(grpc_message)?)),
        }
    }

    pub fn use_cache(&self) -> bool {
        match self {
            DexType::RaydiumAMM => true,
            DexType::RaydiumCLmm => false,
            DexType::PumpFunAMM => true,
            DexType::MeteoraDLMM => false,
        }
    }

    // TODO : 启动初始化，单例
    pub fn get_quoter(&self) -> Box<dyn Quoter> {
        match self {
            DexType::RaydiumAMM => Box::new(RaydiumAmmDex),
            DexType::RaydiumCLmm => Box::new(RaydiumCLMMDex),
            DexType::PumpFunAMM => Box::new(PumpFunDex),
            DexType::MeteoraDLMM => Box::new(MeteoraDLMMDex),
        }
    }

    pub fn get_instruction_item_creator(&self) -> Box<dyn InstructionItemCreator> {
        match self {
            DexType::RaydiumAMM => Box::new(RaydiumAmmDex),
            DexType::RaydiumCLmm => Box::new(RaydiumCLMMDex),
            DexType::PumpFunAMM => Box::new(PumpFunDex),
            DexType::MeteoraDLMM => Box::new(MeteoraDLMMDex),
        }
    }

    pub fn get_account_meta_converter(&self) -> Box<dyn AccountMetaConverter> {
        match self {
            DexType::RaydiumAMM => Box::new(RaydiumAmmDex),
            DexType::RaydiumCLmm => Box::new(RaydiumCLMMDex),
            DexType::PumpFunAMM => Box::new(PumpFunDex),
            DexType::MeteoraDLMM => Box::new(MeteoraDLMMDex),
        }
    }
}

#[derive(Debug, Clone)]
pub enum InstructionItem {
    RaydiumAMM(RaydiumAMMInstructionItem),
    RaydiumCLMM(RaydiumCLMMInstructionItem),
    PumpFunAMM(PumpFunInstructionItem),
    MeteoraDLMM(MeteoraDLMMInstructionItem),
}

impl InstructionItem {
    pub fn get_swap_type(&self) -> Swap {
        match self {
            InstructionItem::RaydiumAMM(_) => Raydium,
            InstructionItem::RaydiumCLMM(_) => RaydiumClmm,
            InstructionItem::PumpFunAMM(item) => {
                if item.zero_to_one {
                    Swap::PumpdotfunAmmSell
                } else {
                    Swap::PumpdotfunAmmBuy
                }
            }
            InstructionItem::MeteoraDLMM(_) => Swap::MeteoraDlmm,
        }
    }

    pub fn parse_account_meta(
        self,
        wallet: Pubkey,
    ) -> Option<(Vec<AccountMeta>, Vec<AddressLookupTableAccount>)> {
        let converter = match &self {
            InstructionItem::RaydiumAMM(_) => DexType::RaydiumAMM.get_account_meta_converter(),
            InstructionItem::RaydiumCLMM(_) => DexType::RaydiumCLmm.get_account_meta_converter(),
            InstructionItem::PumpFunAMM(_) => DexType::PumpFunAMM.get_account_meta_converter(),
            InstructionItem::MeteoraDLMM(_) => DexType::MeteoraDLMM.get_account_meta_converter(),
        };
        converter.converter(wallet, self)
    }
}

#[derive(Debug, Clone)]
pub enum GrpcMessage {
    RaydiumAMMData {
        pool_id: Pubkey,
        mint_0_vault_amount: Option<u64>,
        mint_1_vault_amount: Option<u64>,
        mint_0_need_take_pnl: Option<u64>,
        mint_1_need_take_pnl: Option<u64>,
    },
    RaydiumCLMMData {
        pool_id: Pubkey,
        tick_current: i32,
        liquidity: u128,
        sqrt_price_x64: u128,
        tick_array_bitmap: [u64; 16],
    },
    PumpFunAMMData {
        pool_id: Pubkey,
        mint_0_vault_amount: Option<u64>,
        mint_1_vault_amount: Option<u64>,
    },
    MeteoraDLMMData {
        pool_id: Pubkey,
        active_id: i32,
        bin_array_bitmap: [u64; 16],
        volatility_accumulator: u32,
        volatility_reference: u32,
        index_reference: i32,
        last_update_timestamp: i64,
    },
    Clock(Clock),
}

impl GrpcMessage {
    pub fn pool_id(&self) -> Option<Pubkey> {
        match self {
            GrpcMessage::RaydiumAMMData { pool_id, .. } => Some(*pool_id),
            GrpcMessage::RaydiumCLMMData { pool_id, .. } => Some(*pool_id),
            GrpcMessage::PumpFunAMMData { pool_id, .. } => Some(*pool_id),
            GrpcMessage::MeteoraDLMMData { pool_id, .. } => Some(*pool_id),
            GrpcMessage::Clock(_) => unimplemented!(),
        }
    }
}

#[derive(Debug, Clone)]
pub enum SourceMessage {
    Account(AccountUpdate),
    Clock(ClockUpdate),
}

#[derive(Debug, Clone)]
pub struct ClockUpdate {
    pub account: SubscribeUpdateAccount,
}

#[derive(Debug, Clone)]
pub struct AccountUpdate {
    pub protocol: DexType,
    pub account_type: GrpcAccountUpdateType,
    pub filters: Vec<String>,
    pub account: SubscribeUpdateAccount,
}

impl
    From<(
        DexType,
        GrpcAccountUpdateType,
        Vec<String>,
        SubscribeUpdateAccount,
    )> for SourceMessage
{
    fn from(
        value: (
            DexType,
            GrpcAccountUpdateType,
            Vec<String>,
            SubscribeUpdateAccount,
        ),
    ) -> Self {
        Account(AccountUpdate {
            protocol: value.0,
            account_type: value.1,
            filters: value.2,
            account: value.3,
        })
    }
}

#[derive(Eq, PartialEq, Hash, Debug, Clone)]
pub enum GrpcAccountUpdateType {
    Pool,
    MintVault,
    Clock,
}

pub trait ReadyGrpcMessageOperator {
    fn parse_message(&mut self) -> Result<()>;

    fn change_and_return_ready_data(&self, old: &mut GrpcMessage) -> Result<()>;

    fn get_cache_key(&self) -> (String, Pubkey);

    fn get_insert_data(&self) -> GrpcMessage;
}

pub trait GrpcSubscribeRequestGenerator {
    fn create_subscribe_requests(
        &self,
        pools: &[Pool],
    ) -> Option<Vec<(SubscribeKey, SubscribeRequest)>>;

    fn mint_vault_subscribe_request(&self, pools: &[Pool]) -> SubscribeRequest {
        SubscribeRequest {
            accounts: pools
                .iter()
                .filter_map(|pool| {
                    let pool_id = pool.pool_id;
                    if let Some((mint_0_vault, mint_1_vault)) = pool.mint_vault_pair() {
                        Some([
                            (
                                // mint_vault账户上没有关联的pool_id信息
                                // 通过filter_name在grpc推送消息时确定关联的pool
                                format!("{}:{}", pool_id, 0),
                                SubscribeRequestFilterAccounts {
                                    account: vec![mint_0_vault.to_string()],
                                    ..Default::default()
                                },
                            ),
                            (
                                format!("{}:{}", pool_id, 1),
                                SubscribeRequestFilterAccounts {
                                    account: vec![mint_1_vault.to_string()],
                                    ..Default::default()
                                },
                            ),
                        ])
                    } else {
                        None
                    }
                })
                .flatten()
                .collect::<HashMap<_, _>>(),
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
                // state
                SubscribeRequestAccountsDataSlice {
                    offset: 108,
                    length: 1,
                },
            ],
            ..Default::default()
        }
    }
}

#[async_trait::async_trait]
pub trait AccountSnapshotFetcher: Send + Sync {
    async fn fetch_snapshot(
        &self,
        pool_ids: Vec<DexJson>,
        rpc_client: Arc<RpcClient>,
    ) -> Option<Vec<Pool>>;

    async fn load_lookup_table_accounts(
        &self,
        rpc_client: Arc<RpcClient>,
        dex_jsons: Arc<Vec<DexJson>>,
    ) -> Result<HashMap<Pubkey, AddressLookupTableAccount>> {
        let all_alts = dex_jsons
            .iter()
            .map(|json| json.address_lookup_table_address.unwrap())
            .collect::<Vec<_>>();
        let mut join_set = JoinSet::new();
        for alts in all_alts.chunks(100) {
            let alts = alts.to_vec();
            let rpc_client = rpc_client.clone();
            join_set.spawn(async move {
                match rpc_client.get_multiple_accounts(alts.as_slice()).await {
                    Ok(alt_accounts) => alt_accounts
                        .into_iter()
                        .zip(alts)
                        .flat_map(|(account, pubkey)| match account {
                            None => None,
                            Some(account) => match AddressLookupTable::deserialize(&account.data) {
                                Ok(lookup_table) => {
                                    let lookup_table_account = AddressLookupTableAccount {
                                        key: pubkey,
                                        addresses: lookup_table.addresses.into_owned(),
                                    };
                                    Some((pubkey, lookup_table_account))
                                }
                                Err(e) => {
                                    error!(
                                        "   Failed to deserialize lookup table {}: {}",
                                        pubkey, e
                                    );
                                    None
                                }
                            },
                        })
                        .collect::<HashMap<_, _>>(),
                    Err(_) => HashMap::default(),
                }
            });
        }
        let mut alt_map = HashMap::with_capacity(all_alts.len());
        while let Some(Ok(alt)) = join_set.join_next().await {
            alt_map.extend(alt);
        }
        if alt_map.is_empty() {
            Err(anyhow!("未找到任何【AddressLookupTableAccount】"))
        } else {
            Ok(alt_map)
        }
    }
}

pub trait CacheUpdater: Send + Sync {
    fn update_cache(&self, pool: &mut Pool) -> Result<()>;
}

#[async_trait::async_trait]
pub trait DB: Send + Sync {
    async fn load_token_pools(&self) -> Result<Vec<Pool>>;
}

#[async_trait::async_trait]
pub trait Dex: Send + Sync + Debug {
    async fn quote(&self, amount_in: u64) -> Option<u64>;

    fn clone_self(&self) -> Box<dyn Dex>;

    async fn to_instruction_item(
        &self,
        alt_cache: Arc<HashMap<Pubkey, AddressLookupTableAccount>>,
    ) -> InstructionItem;
}

#[async_trait::async_trait]
pub trait Quoter: Send + Sync {
    async fn quote(
        &self,
        amount_in: u64,
        in_mint: Pubkey,
        out_mint: Pubkey,
        pool: &Pool,
        clock: Arc<Clock>,
    ) -> Option<u64>;
}

pub trait InstructionItemCreator {
    fn create_instruction_item(&self, pool: &Pool, in_mint: &Pubkey) -> Option<InstructionItem>;
}

pub trait AccountMetaConverter {
    fn converter(
        &self,
        wallet: Pubkey,
        instruction_item: InstructionItem,
    ) -> Option<(Vec<AccountMeta>, Vec<AddressLookupTableAccount>)>;
}
