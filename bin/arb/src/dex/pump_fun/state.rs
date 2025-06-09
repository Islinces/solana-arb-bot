use crate::account_cache::{DynamicCache, StaticCache};
use crate::dex::byte_utils::{read_pubkey, read_u64};
use crate::dex::FromCache;
use crate::interface1::DexType;
use parking_lot::RwLockReadGuard;
use solana_sdk::pubkey::Pubkey;

/// 字段顺序不要动
#[derive(Default, Debug)]
pub struct Pool {
    // ============= static data =================
    // 池子
    pub base_mint: Pubkey,
    pub quote_mint: Pubkey,
    pub pool_base_token_account: Pubkey,
    pub pool_quote_token_account: Pubkey,
    pub coin_creator: Pubkey,
    // 全局配置，fee相关
    pub lp_fee_basis_points: u64,
    pub protocol_fee_basis_points: u64,
    // 池子data里没有，在初始化缓存的时候计算之后设置进来的
    pub coin_creator_vault_authority: Pubkey,
    pub coin_creator_vault_ata: Pubkey,
    // ============= dynamic data =================
    // 金库
    pub base_vault_amount: u64,
    pub quote_vault_amount: u64,
}

impl FromCache for Pool {
    fn from_cache(
        pool_id: &Pubkey,
        static_cache: RwLockReadGuard<StaticCache>,
        dynamic_cache: &DynamicCache,
    ) -> Option<Self>
    where
        Self: Sized,
    {
        let pool_static_data = static_cache.get(pool_id)?;
        unsafe {
            let base_mint = read_pubkey(&pool_static_data[0..32]);
            let quote_mint = read_pubkey(&pool_static_data[32..64]);
            let pool_base_token_account = read_pubkey(&pool_static_data[64..96]);
            let pool_quote_token_account = read_pubkey(&pool_static_data[96..128]);
            let coin_creator = read_pubkey(&pool_static_data[128..160]);

            let lp_fee_basis_points = read_u64(&pool_static_data[160..168]);
            let protocol_fee_basis_points = read_u64(&pool_static_data[168..176]);

            let coin_creator_vault_authority = read_pubkey(&pool_static_data[176..208]);
            let coin_creator_vault_ata = read_pubkey(&pool_static_data[208..240]);

            let base_vault_amount_bytes = dynamic_cache.get(&pool_base_token_account)?;
            let quote_vault_amount_bytes = dynamic_cache.get(&pool_quote_token_account)?;

            let base_vault_amount = read_u64(&base_vault_amount_bytes[0..8]);
            let quote_vault_amount = read_u64(&quote_vault_amount_bytes[0..8]);

            Some(Self {
                base_mint,
                quote_mint,
                pool_base_token_account,
                pool_quote_token_account,
                coin_creator,
                lp_fee_basis_points,
                protocol_fee_basis_points,
                coin_creator_vault_authority,
                coin_creator_vault_ata,
                base_vault_amount,
                quote_vault_amount,
            })
        }
    }
}

pub fn global_config_key() -> Pubkey {
    Pubkey::find_program_address(
        &[b"global_config"],
        DexType::PumpFunAMM.get_ref_program_id(),
    )
    .0
}
