use crate::dex::meteora_dlmm::commons::pda;
use crate::dex::raydium_clmm::state::POOL_TICK_ARRAY_BITMAP_SEED;
use crate::dex_data::DexJson;
use crate::interface;
use crate::interface::{AccountType, DexType};
use ahash::AHashMap;
use anyhow::anyhow;
use solana_sdk::pubkey::Pubkey;
use std::sync::Arc;
use tokio::sync::OnceCell;
use tracing::info;

static ACCOUNT_RELATION_CACHE: OnceCell<Arc<AccountRelation>> = OnceCell::const_new();

#[derive(Debug)]
pub struct AccountRelation {
    pool: AHashMap<Pubkey, DexType>,
    vault: AHashMap<Pubkey, (Pubkey, DexType)>,
    tick_array_extension_bitmap: AHashMap<Pubkey, Pubkey>,
    bin_array_extension_bitmap: AHashMap<Pubkey, Pubkey>,
}

impl AccountRelation {
    fn new(dex_data: &[DexJson]) -> anyhow::Result<Self> {
        info!("初始化AccountRelation...");
        let mut pool = AHashMap::with_capacity(dex_data.len());
        let mut vault = AHashMap::with_capacity(dex_data.len());
        let mut tick_array_extension_bitmap = AHashMap::with_capacity(dex_data.len());
        let mut bin_array_extension_bitmap = AHashMap::with_capacity(dex_data.len());
        for json in dex_data.iter() {
            if let Some(dex_type) = interface::get_dex_type_with_program_id(&json.owner) {
                pool.insert(json.pool, dex_type.clone());
                vault.insert(json.vault_a, (json.pool, dex_type.clone()));
                vault.insert(json.vault_b, (json.pool, dex_type.clone()));
                if DexType::RaydiumCLMM == dex_type {
                    tick_array_extension_bitmap.insert(
                        Pubkey::find_program_address(
                            &[POOL_TICK_ARRAY_BITMAP_SEED.as_bytes(), json.pool.as_ref()],
                            DexType::RaydiumCLMM.get_ref_program_id(),
                        )
                        .0,
                        json.pool,
                    );
                } else if DexType::MeteoraDLMM == dex_type {
                    bin_array_extension_bitmap.insert(
                        pda::derive_bin_array_bitmap_extension(&json.pool),
                        json.pool,
                    );
                }
            }
        }
        info!("初始化AccountRelation结束");
        Ok(Self {
            pool,
            vault,
            tick_array_extension_bitmap,
            bin_array_extension_bitmap,
        })
    }
}

pub(crate) fn init(dex_data: &[DexJson]) -> anyhow::Result<()> {
    ACCOUNT_RELATION_CACHE
        .set(Arc::new(AccountRelation::new(dex_data)?))
        .map_or(Err(anyhow!("初始化AccountRelation失败")), |_| Ok(()))
}

#[inline]
pub fn is_follow_vault(vault_account: &Pubkey) -> Option<(Pubkey, DexType)> {
    match ACCOUNT_RELATION_CACHE.get() {
        None => None,
        Some(cache) => cache.vault.get(vault_account).cloned(),
    }
}

pub fn get_dex_type_and_account_type(
    owner: &Pubkey,
    account_key: &Pubkey,
) -> Option<(DexType, AccountType)> {
    let relation_cache = ACCOUNT_RELATION_CACHE.get().unwrap();
    if owner == &spl_token::ID {
        Some((
            relation_cache.vault.get(account_key)?.1.clone(),
            AccountType::MintVault,
        ))
    } else if owner == DexType::RaydiumAMM.get_ref_program_id() {
        Some((DexType::RaydiumAMM, AccountType::Pool))
    } else if owner == DexType::PumpFunAMM.get_ref_program_id() {
        Some((DexType::PumpFunAMM, AccountType::Pool))
    } else if owner == DexType::RaydiumCLMM.get_ref_program_id() {
        if relation_cache.pool.contains_key(account_key) {
            Some((DexType::RaydiumCLMM, AccountType::Pool))
        } else if relation_cache
            .tick_array_extension_bitmap
            .contains_key(account_key)
        {
            Some((DexType::RaydiumCLMM, AccountType::TickArrayBitmap))
        } else {
            Some((DexType::RaydiumCLMM, AccountType::TickArray))
        }
    } else if owner == DexType::MeteoraDLMM.get_ref_program_id() {
        if relation_cache.pool.contains_key(account_key) {
            Some((DexType::MeteoraDLMM, AccountType::Pool))
        } else if relation_cache
            .bin_array_extension_bitmap
            .contains_key(account_key)
        {
            Some((DexType::MeteoraDLMM, AccountType::BinArrayBitmap))
        } else {
            Some((DexType::MeteoraDLMM, AccountType::BinArray))
        }
    } else {
        None
    }
}
