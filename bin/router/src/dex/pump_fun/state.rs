use anchor_lang::prelude::*;
use solana_program::pubkey::Pubkey;

#[account]
#[derive(Default)]
pub struct Pool {
    pub pool_bump: u8,
    pub index: u16,
    pub creator: Pubkey,
    pub base_mint: Pubkey,
    pub quote_mint: Pubkey,
    pub lp_mint: Pubkey,
    pub pool_base_token_account: Pubkey,
    pub pool_quote_token_account: Pubkey,
    pub lp_supply: u64,
}

#[account]
#[derive(Default, Debug)]
pub struct GlobalConfig {
    pub admin: Pubkey,
    pub lp_fee_basis_points: u64,
    pub protocol_fee_basis_points: u64,
    pub disable_flags: u8,
    pub protocol_fee_recipients: [Pubkey; 8],
}

impl GlobalConfig {
    
    pub fn key()->Pubkey{
        Pubkey::find_program_address(&[b"global_config"], &crate::dex::pump_fun::ID).0
    }
    
}