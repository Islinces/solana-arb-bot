use bytemuck::{Pod, Zeroable};
use solana_sdk::pubkey::Pubkey;

#[repr(C, packed)]
#[derive(Debug, Clone, Default)]
#[cfg_attr(test, derive(Copy, Pod, Zeroable))]
pub struct Pool {
    pub discriminator: [u8; 8],
    pub pool_bump: u8,
    pub index: u16,
    pub creator: Pubkey,
    pub base_mint: Pubkey,
    pub quote_mint: Pubkey,
    pub lp_mint: Pubkey,
    pub pool_base_token_account: Pubkey,
    pub pool_quote_token_account: Pubkey,
    pub lp_supply: u64,
    pub coin_creator: Pubkey,
}
