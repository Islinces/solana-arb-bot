use crate::account_cache::{DynamicCache, StaticCache};
use crate::dex::byte_utils::{read_pubkey, read_u64};
use crate::dex::FromCache;
use parking_lot::RwLockReadGuard;
use solana_sdk::pubkey::Pubkey;

#[repr(C, packed)]
#[derive(Clone, Copy, Default, PartialEq, Debug)]
pub struct AmmInfo {
    // 分开存储
    // static data 未订阅的属性
    pub swap_fee_numerator: u64,
    pub swap_fee_denominator: u64,
    pub coin_vault: Pubkey,
    pub pc_vault: Pubkey,
    pub coin_vault_mint: Pubkey,
    pub pc_vault_mint: Pubkey,
    // dynamic data 订阅的属性
    pub need_take_pnl_coin: u64,
    pub need_take_pnl_pc: u64,
    // dynamic data 金库
    pub coin_vault_amount: u64,
    pub pc_vault_amount: u64,
}

impl FromCache for AmmInfo {
    fn from_cache(
        pool_id: &Pubkey,
        static_cache: RwLockReadGuard<StaticCache>,
        dynamic_cache: &DynamicCache,
    ) -> Option<Self>
    where
        Self: Sized,
    {
        let pool_static_data = static_cache.get(pool_id)?;
        let pool_dynamic_data_ref = dynamic_cache.get(pool_id)?;
        let pool_dynamic_data = pool_dynamic_data_ref.value().as_slice();

        unsafe {
            let swap_fee_numerator = read_u64(&pool_static_data[0..8]);
            let swap_fee_denominator = read_u64(&pool_static_data[8..16]);
            let coin_vault = read_pubkey(&pool_static_data[16..48]);
            let pc_vault = read_pubkey(&pool_static_data[48..80]);
            let coin_vault_mint = read_pubkey(&pool_static_data[80..112]);
            let pc_vault_mint = read_pubkey(&pool_static_data[112..144]);

            let need_take_pnl_coin = read_u64(&pool_dynamic_data[0..8]);
            let need_take_pnl_pc = read_u64(&pool_dynamic_data[8..16]);

            let coin_vault_amount_bytes = dynamic_cache.get(&coin_vault)?;
            let pc_vault_amount_bytes = dynamic_cache.get(&pc_vault)?;

            let coin_vault_amount = read_u64(&coin_vault_amount_bytes[0..8]);
            let pc_vault_amount = read_u64(&pc_vault_amount_bytes[0..8]);

            Some(Self {
                swap_fee_numerator,
                swap_fee_denominator,
                coin_vault,
                pc_vault,
                coin_vault_mint,
                pc_vault_mint,
                need_take_pnl_coin,
                need_take_pnl_pc,
                coin_vault_amount,
                pc_vault_amount,
            })
        }
    }
}
