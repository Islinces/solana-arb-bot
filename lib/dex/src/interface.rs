use solana_program::pubkey::Pubkey;
use std::fmt::Debug;

pub trait Dex {
    fn get_pools(&self) -> Vec<Box<dyn Pool>>;
}

pub trait Pool: Send + Sync + Debug {
    fn get_pool_id(&self) -> Pubkey;
    fn get_mint_0(&self) -> Pubkey;
    fn get_mint_1(&self) -> Pubkey;
    fn quote(&self, amount_in: u64, amount_in_mint: Pubkey) -> u64;
    fn clone_box(&self) -> Box<dyn Pool>;
}
