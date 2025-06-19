use crate::dex::utils::read_from;
use crate::dex::FromCache;
use anyhow::anyhow;
use solana_sdk::pubkey::Pubkey;
use std::sync::Arc;

#[repr(C, packed)]
// #[serde_as]
#[derive(Clone, Copy, Default, PartialEq, Debug)]
// #[cfg_attr(feature = "print_data_after_update", derive(Serialize, Deserialize))]
pub struct AmmInfo {
    // 分开存储
    // static data 未订阅的属性
    pub swap_fee_numerator: u64,
    pub swap_fee_denominator: u64,
    // #[serde_as(as = "DisplayFromStr")]
    pub coin_vault: Pubkey,
    // #[serde_as(as = "DisplayFromStr")]
    pub pc_vault: Pubkey,
    // #[serde_as(as = "DisplayFromStr")]
    pub coin_vault_mint: Pubkey,
    // #[serde_as(as = "DisplayFromStr")]
    pub pc_vault_mint: Pubkey,
    // dynamic data 订阅的属性
    pub need_take_pnl_coin: u64,
    pub need_take_pnl_pc: u64,
    // // dynamic data 金库
    // pub coin_vault_amount: u64,
    // pub pc_vault_amount: u64,
}

impl FromCache for AmmInfo {
    fn from_cache(
        static_cache: Option<Arc<Vec<u8>>>,
        dynamic_cache: Option<Arc<Vec<u8>>>,
    ) -> anyhow::Result<Self>
    where
        Self: Sized,
    {
        let pool_static_data = static_cache.ok_or(anyhow!(""))?;
        let pool_dynamic_data = dynamic_cache.ok_or(anyhow!(""))?;
        let pool_static_data=pool_static_data.as_slice();
        let pool_dynamic_data=pool_dynamic_data.as_slice();
        unsafe {
            let swap_fee_numerator = read_from::<u64>(&pool_static_data[0..8]);
            let swap_fee_denominator = read_from::<u64>(&pool_static_data[8..16]);
            let coin_vault = read_from::<Pubkey>(&pool_static_data[16..48]);
            let pc_vault = read_from::<Pubkey>(&pool_static_data[48..80]);
            let coin_vault_mint = read_from::<Pubkey>(&pool_static_data[80..112]);
            let pc_vault_mint = read_from::<Pubkey>(&pool_static_data[112..144]);

            let need_take_pnl_coin = read_from::<u64>(&pool_dynamic_data[0..8]);
            let need_take_pnl_pc = read_from::<u64>(&pool_dynamic_data[8..16]);
            Ok(Self {
                swap_fee_numerator,
                swap_fee_denominator,
                coin_vault,
                pc_vault,
                coin_vault_mint,
                pc_vault_mint,
                need_take_pnl_coin,
                need_take_pnl_pc,
            })
        }
    }
}
