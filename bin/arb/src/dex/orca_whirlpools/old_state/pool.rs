use solana_sdk::pubkey::Pubkey;

#[derive(Clone, Debug)]
pub struct Whirlpool {
    pub discriminator: [u8; 8],
    pub whirlpools_config: Pubkey,
    pub whirlpool_bump: [u8; 1],
    pub tick_spacing: u16,
    pub fee_tier_index_seed: [u8; 2],
    pub fee_rate: u16,
    pub protocol_fee_rate: u16,
    pub liquidity: u128,
    pub sqrt_price: u128,
    pub tick_current_index: i32,
    pub protocol_fee_owed_a: u64,
    pub protocol_fee_owed_b: u64,
    pub token_mint_a: Pubkey,
    pub token_vault_a: Pubkey,
    pub fee_growth_global_a: u128,
    pub token_mint_b: Pubkey,
    pub token_vault_b: Pubkey,
    pub fee_growth_global_b: u128,
    pub reward_last_updated_timestamp: u64,
    pub reward_infos: [WhirlpoolRewardInfo; 3],
}

impl TryInto<crate::dex::orca_whirlpools::accounts::whirlpool::Whirlpool> for Whirlpool {
    type Error = anyhow::Error;

    fn try_into(self) -> Result<crate::dex::whirlpool::Whirlpool, Self::Error> {
        Ok(crate::dex::whirlpool::Whirlpool {
            tick_spacing: self.tick_spacing,
            fee_tier_index_seed: self.fee_tier_index_seed,
            fee_rate: self.fee_rate,
            liquidity: self.liquidity,
            sqrt_price: self.sqrt_price,
            tick_current_index: self.tick_current_index,
            protocol_fee_owed_a: self.protocol_fee_owed_a,
            protocol_fee_owed_b: self.protocol_fee_owed_b,
            token_mint_a: self.token_mint_a,
            token_vault_a: self.token_vault_a,
            token_mint_b: self.token_mint_b,
            token_vault_b: self.token_vault_b,
        })
    }
}

#[derive(Clone, Debug)]
pub struct WhirlpoolRewardInfo {
    /// Reward token mint.
    pub mint: Pubkey,
    /// Reward vault token account.
    pub vault: Pubkey,
    /// Authority account that has permission to initialize the reward and set emissions.
    pub authority: Pubkey,
    /// Q64.64 number that indicates how many tokens per second are earned per unit of liquidity.
    pub emissions_per_second_x64: u128,
    /// Q64.64 number that tracks the total tokens earned per unit of liquidity since the reward
    /// emissions were turned on.
    pub growth_global_x64: u128,
}
