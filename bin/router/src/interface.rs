use crate::cache::init_cache;
use crate::data_slice::init_data_slice_config;
use crate::dex_data::DexJson;
use crate::state::DexMetadata;
use anyhow::anyhow;
use solana_rpc_client::nonblocking::rpc_client::RpcClient;
use solana_sdk::pubkey;
use solana_sdk::pubkey::Pubkey;
use std::fmt::{Debug, Display, Formatter};
use std::fs::File;
use std::sync::Arc;
use tokio::sync::OnceCell;
use tracing::error;

pub const RAYDIUM_AMM_PROGRAM_ID: Pubkey = pubkey!("675kPX9MHTjS2zt1qfr1NYHuzeLXfQM9H24wFSUt1Mp8");
pub const RAYDIUM_AMM_VAULT_OWNER: Pubkey = pubkey!("5Q544fKrFoe6tsEbD7S8EmxGTJYAKtTVhAW5Q5pge4j1");
pub const RAYDIUM_CLMM_PROGRAM_ID: Pubkey = pubkey!("CAMMCzo5YL8w4VFF8KVHrK22GGUsp5VTaW7grrKgrWqK");
pub const PUMP_FUN_AMM_PROGRAM_ID: Pubkey = pubkey!("pAMMBay6oceH9fJKBRHGP5D4bD4sWpmSwMn52FMfXEA");
pub const METEORA_DLMM_PROGRAM_ID: Pubkey = pubkey!("LBUZKhRxPF3XUpBCjp4YzTKgLccjZhTSDM9YuVaPwxo");

pub const ATA_PROGRAM_ID: Pubkey = pubkey!("LBUZKhRxPF3XUpBCjp4YzTKgLccjZhTSDM9YuVaPwxo");
pub const SYSTEM_PROGRAM: Pubkey = pubkey!("11111111111111111111111111111111");

pub static DEX_METADATA: OnceCell<DexMetadata> = OnceCell::const_new();

#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub enum DexType {
    RaydiumAMM,
    RaydiumCLMM,
    PumpFunAMM,
    // MeteoraDLMM,
}

#[derive(Debug, Clone)]
pub enum AccountType {
    Pool,
    MintVault,
    TickArrayState,
    TickArrayBitmapExtension,
    PumpFunGlobalConfig,
}

impl Display for DexType {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        f.write_str(match self {
            DexType::RaydiumAMM => "RaydiumAMM",
            DexType::RaydiumCLMM => "RaydiumCLmm",
            DexType::PumpFunAMM => "PumpFunAMM",
        })
    }
}

impl DexType {
    pub fn get_ref_program_id(&self) -> &Pubkey {
        match self {
            DexType::RaydiumAMM => &RAYDIUM_AMM_PROGRAM_ID,
            DexType::RaydiumCLMM => &RAYDIUM_CLMM_PROGRAM_ID,
            DexType::PumpFunAMM => &PUMP_FUN_AMM_PROGRAM_ID,
        }
    }
}

#[inline]
pub fn is_follow_vault(vault_account: &Pubkey) -> Option<(Pubkey, DexType)> {
    DEX_METADATA
        .get()
        .unwrap()
        .get_dex_type_and_pool_id_for_vault(vault_account)
}

#[inline]
pub fn get_dex_type_with_program_id(program_id: &Pubkey) -> Option<DexType> {
    if program_id == &RAYDIUM_CLMM_PROGRAM_ID {
        Some(DexType::RaydiumCLMM)
    } else if program_id == &RAYDIUM_AMM_PROGRAM_ID {
        Some(DexType::RaydiumAMM)
    } else if program_id == &PUMP_FUN_AMM_PROGRAM_ID {
        Some(DexType::PumpFunAMM)
    } else {
        None
    }
}

pub async fn init_start_data(
    dex_json_path: String,
    rpc_client: Arc<RpcClient>,
) -> anyhow::Result<Vec<DexJson>> {
    let dex_data: Vec<DexJson> = match File::open(dex_json_path.as_str()) {
        Ok(file) => serde_json::from_reader(file).expect("解析【dex_data.json】失败"),
        Err(e) => {
            error!("{}", e);
            vec![]
        }
    };
    if dex_data.is_empty() {
        Err(anyhow!("json文件无数据"))
    } else {
        init_data_slice_config();
        let dex_data = init_cache(dex_data, rpc_client).await?;
        DEX_METADATA.set(DexMetadata::new(&dex_data)?)?;
        Ok(dex_data)
    }
}
