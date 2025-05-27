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
}