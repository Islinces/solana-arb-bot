use solana_sdk::pubkey::Pubkey;

#[repr(C, packed)]
#[derive(Default, Debug)]
pub struct PoolState {
    /// Which config the pool belongs
    /// 8,32
    pub amm_config: Pubkey,
    /// pool creator
    /// 40,32
    pub pool_creator: Pubkey,
    /// Token A
    /// 72,32
    pub token_0_vault: Pubkey,
    /// Token B
    /// 104,32
    pub token_1_vault: Pubkey,

    /// Pool tokens are issued when A or B tokens are deposited.
    /// Pool tokens can be withdrawn back to the original A or B token.
    /// 136,32
    pub lp_mint: Pubkey,
    /// Mint information for token A
    /// 168,32
    pub token_0_mint: Pubkey,
    /// Mint information for token B
    /// 200,32
    pub token_1_mint: Pubkey,

    /// token_0 program
    /// 232,32
    pub token_0_program: Pubkey,
    /// token_1 program
    /// 264，32
    pub token_1_program: Pubkey,

    /// observation account to store oracle data
    /// 296，32
    pub observation_key: Pubkey,
    // 328，329
    pub auth_bump: u8,
    /// Bitwise representation of the state of the pool
    /// bit0, 1: disable deposit(value is 1), 0: normal
    /// bit1, 1: disable withdraw(value is 2), 0: normal
    /// bit2, 1: disable swap(value is 4), 0: normal
    /// 329，330
    pub status: u8,
    // 330，331
    pub lp_mint_decimals: u8,
    /// mint0 and mint1 decimals
    /// 331，332
    pub mint_0_decimals: u8,
    // 332，333
    pub mint_1_decimals: u8,

    /// True circulating supply without burns and lock ups
    /// 333，341
    pub lp_supply: u64,
    /// The amounts of token_0 and token_1 that are owed to the liquidity provider.
    /// 341，349
    pub protocol_fees_token_0: u64,
    // 349，357
    pub protocol_fees_token_1: u64,
    // 357，365
    pub fund_fees_token_0: u64,
    // 365，373
    pub fund_fees_token_1: u64,

    /// The timestamp allowed for swap in the pool.
    /// 373，8
    pub open_time: u64,
    // 381，8
    /// recent epoch
    pub recent_epoch: u64,
    // 389，31*8
    /// padding for future updates
    pub padding: [u64; 31],
}

impl TryInto<crate::dex::raydium_cpmm::states::PoolState> for PoolState {
    type Error = anyhow::Error;

    fn try_into(self) -> Result<crate::dex::raydium_cpmm::states::PoolState, Self::Error> {
        Ok(crate::dex::raydium_cpmm::states::PoolState {
            amm_config: self.amm_config,
            token_0_vault: self.token_0_vault,
            token_1_vault: self.token_1_vault,
            token_0_mint: self.token_0_mint,
            token_1_mint: self.token_1_mint,
            token_0_program: self.token_0_program,
            token_1_program: self.token_1_program,
            observation_key: self.observation_key,
            status: self.status,
            protocol_fees_token_0: self.protocol_fees_token_0,
            protocol_fees_token_1: self.protocol_fees_token_1,
            fund_fees_token_0: self.fund_fees_token_0,
            fund_fees_token_1: self.fund_fees_token_1,
            open_time: self.open_time,
        })
    }
}
