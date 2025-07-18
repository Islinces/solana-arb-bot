use crate::dex::meteora_damm_v2::old_state::fee::PoolFeesStruct;
use solana_sdk::pubkey::Pubkey;

const NUM_REWARDS: usize = 2;

#[repr(C, packed)]
#[derive(Debug, Clone, Copy)]
pub struct Pool {
    /// Pool fee
    pub pool_fees: PoolFeesStruct,
    /// token a mint
    pub token_a_mint: Pubkey,
    /// token b mint
    pub token_b_mint: Pubkey,
    /// token a vault
    pub token_a_vault: Pubkey,
    /// token b vault
    pub token_b_vault: Pubkey,
    /// Whitelisted vault to be able to buy pool before activation_point
    pub whitelisted_vault: Pubkey,
    /// partner
    pub partner: Pubkey,
    /// liquidity share
    /// 360,16
    pub liquidity: u128,
    /// padding, previous reserve amount, be careful to use that field
    /// 376,16
    pub _padding: u128,
    /// protocol a fee
    /// 392,8
    pub protocol_a_fee: u64,
    /// protocol b fee
    /// 400,8
    pub protocol_b_fee: u64,
    /// partner a fee
    /// 408,8
    pub partner_a_fee: u64,
    /// partner b fee
    /// 416,8
    pub partner_b_fee: u64,
    /// min price
    /// 424,16
    pub sqrt_min_price: u128,
    /// max price
    /// 440,16
    pub sqrt_max_price: u128,
    /// current price
    /// 456,16
    pub sqrt_price: u128,
    /// Activation point, can be slot or timestamp
    /// 472,8
    pub activation_point: u64,
    /// Activation type, 0 means by slot, 1 means by timestamp
    /// 480,1
    pub activation_type: u8,
    /// pool status, 0: enable, 1 disable
    /// 481,1
    pub pool_status: u8,
    /// token a flag
    /// 482,1
    pub token_a_flag: u8,
    /// token b flag
    /// 483,1
    pub token_b_flag: u8,
    /// 0 is collect fee in both token, 1 only collect fee in token a, 2 only collect fee in token b
    /// 484,1
    pub collect_fee_mode: u8,
    /// pool type
    pub pool_type: u8,
    /// padding
    pub _padding_0: [u8; 2],
    /// cumulative
    pub fee_a_per_liquidity: [u8; 32], // U256
    /// cumulative
    pub fee_b_per_liquidity: [u8; 32], // U256
    // TODO: Is this large enough?
    pub permanent_lock_liquidity: u128,
    /// metrics
    pub metrics: PoolMetrics,
    /// Padding for further use
    pub _padding_1: [u64; 10],
    /// Farming reward information
    pub reward_infos: [RewardInfo; NUM_REWARDS],
}

impl TryInto<crate::dex::meteora_damm_v2::state::pool::Pool> for Pool {
    type Error = anyhow::Error;

    fn try_into(self) -> Result<crate::dex::meteora_damm_v2::state::pool::Pool, Self::Error> {
        let base_fees = self.pool_fees;
        Ok(crate::dex::meteora_damm_v2::state::pool::Pool {
            base_fee: base_fees.base_fee.try_into()?,
            dynamic_fee: base_fees.dynamic_fee.try_into()?,
            token_a_mint: self.token_a_mint,
            token_b_mint: self.token_b_mint,
            token_a_vault: self.token_a_vault,
            token_b_vault: self.token_b_vault,
            liquidity: self.liquidity,
            sqrt_min_price: self.sqrt_min_price,
            sqrt_max_price: self.sqrt_max_price,
            sqrt_price: self.sqrt_price,
            activation_point: self.activation_point,
            activation_type: self.activation_type,
            pool_status: self.pool_status,
            token_a_flag: self.token_a_flag,
            token_b_flag: self.token_b_flag,
            collect_fee_mode: self.collect_fee_mode,
        })
    }
}

#[repr(C, packed)]
#[derive(Debug, Clone, Copy)]
pub struct PoolMetrics {
    pub total_lp_a_fee: u128,
    pub total_lp_b_fee: u128,
    pub total_protocol_a_fee: u64,
    pub total_protocol_b_fee: u64,
    pub total_partner_a_fee: u64,
    pub total_partner_b_fee: u64,
    pub total_position: u64,
    pub padding: u64,
}

#[repr(C, packed)]
#[derive(Debug, Clone, Copy)]
pub struct RewardInfo {
    /// Indicates if the reward has been initialized
    pub initialized: u8,
    /// reward token flag
    pub reward_token_flag: u8,
    /// padding
    pub _padding_0: [u8; 6],
    /// Padding to ensure `reward_rate: u128` is 16-byte aligned
    pub _padding_1: [u8; 8], // 8 bytes
    /// Reward token mint.
    pub mint: Pubkey,
    /// Reward vault token account.
    pub vault: Pubkey,
    /// Authority account that allows to fund rewards
    pub funder: Pubkey,
    /// reward duration
    pub reward_duration: u64,
    /// reward duration end
    pub reward_duration_end: u64,
    /// reward rate
    pub reward_rate: u128,
    /// Reward per token stored
    pub reward_per_token_stored: [u8; 32], // U256
    /// The last time reward states were updated.
    pub last_update_time: u64,
    /// Accumulated seconds when the farm distributed rewards but the bin was empty.
    /// These rewards will be carried over to the next reward time window.
    pub cumulative_seconds_with_empty_liquidity_reward: u64,
}
