use crate::defi::raydium_clmm::sdk::config;
use serde::Deserialize;
use solana_program::pubkey::Pubkey;

#[derive(Debug, Clone, Deserialize)]
pub struct ClmmPoolInfo {
    pub amm_config_index: u16,
    #[serde(deserialize_with = "crate::defi::common::utils::deserialize_pubkey")]
    pub mint_0: Pubkey,
    pub mint_0_decimals: u8,
    #[serde(deserialize_with = "crate::defi::common::utils::deserialize_pubkey")]
    pub mint_1: Pubkey,
    pub mint_1_decimals: u8,
    #[serde(skip_deserializing)]
    pub amm_config_key: Pubkey,
    #[serde(skip_deserializing)]
    pub pool_id: Pubkey,
    #[serde(skip_deserializing)]
    pub tickarray_bitmap_extension: Pubkey,
}

impl ClmmPoolInfo {
    pub fn calcul_pda(self) -> Option<Vec<Pubkey>> {
        let (amm_config_key, __bump) = Pubkey::find_program_address(
            &[
                config::AMM_CONFIG_SEED.as_bytes(),
                &self.amm_config_index.to_be_bytes(),
            ],
            &crate::defi::raydium_clmm::ID,
        );

        let mut mint_0 = self.mint_0;
        let mut mint_1 = self.mint_1;
        if mint_0 > mint_1 {
            let temp_mint = mint_0;
            mint_0 = mint_1;
            mint_1 = temp_mint;
        };
        let pool_id_account = Some(
            Pubkey::find_program_address(
                &[
                    crate::defi::raydium_clmm::sdk::pool::POOL_SEED.as_bytes(),
                    amm_config_key.to_bytes().as_ref(),
                    mint_0.to_bytes().as_ref(),
                    mint_1.to_bytes().as_ref(),
                ],
                &crate::defi::raydium_clmm::ID,
            )
            .0,
        );
        // 根据 POOL ACCOUNT 计算
        let tickarray_bitmap_extension = if pool_id_account != None {
            Some(
                Pubkey::find_program_address(
                    &[
                        crate::defi::raydium_clmm::sdk::pool::POOL_TICK_ARRAY_BITMAP_SEED
                            .as_bytes(),
                        pool_id_account.unwrap().to_bytes().as_ref(),
                    ],
                    &crate::defi::raydium_clmm::ID,
                )
                .0,
            )
        } else {
            None
        };
        if tickarray_bitmap_extension.is_none() {
            None
        } else {
            Some(vec![
                pool_id_account.unwrap(),
                tickarray_bitmap_extension.unwrap(),
            ])
        }
    }
}
