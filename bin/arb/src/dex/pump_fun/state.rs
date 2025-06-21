use crate::dex::utils::read_from;
use crate::dex::{DexType, FromCache};
use anyhow::anyhow;
use solana_sdk::pubkey::Pubkey;
use std::sync::Arc;

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
}

impl FromCache for Pool {
    fn from_cache(
        static_cache: Option<Arc<Vec<u8>>>,
        _dynamic_cache: Option<Arc<Vec<u8>>>,
    ) -> anyhow::Result<Self>
    where
        Self: Sized,
    {
        let pool_static_data = static_cache.ok_or(anyhow!(""))?;
        let pool_static_data = pool_static_data.as_slice();
        unsafe {
            let base_mint = read_from::<Pubkey>(&pool_static_data[0..32]);
            let quote_mint = read_from::<Pubkey>(&pool_static_data[32..64]);
            let pool_base_token_account = read_from::<Pubkey>(&pool_static_data[64..96]);
            let pool_quote_token_account = read_from::<Pubkey>(&pool_static_data[96..128]);
            let coin_creator = read_from::<Pubkey>(&pool_static_data[128..160]);

            let lp_fee_basis_points = read_from::<u64>(&pool_static_data[160..168]);
            let protocol_fee_basis_points = read_from::<u64>(&pool_static_data[168..176]);

            let coin_creator_vault_authority = read_from::<Pubkey>(&pool_static_data[176..208]);
            let coin_creator_vault_ata = read_from::<Pubkey>(&pool_static_data[208..240]);
            Ok(Self {
                base_mint,
                quote_mint,
                pool_base_token_account,
                pool_quote_token_account,
                coin_creator,
                lp_fee_basis_points,
                protocol_fee_basis_points,
                coin_creator_vault_authority,
                coin_creator_vault_ata,
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

#[test]
fn test(){
    println!("{:?}",global_config_key());
}
