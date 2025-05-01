use solana_program::pubkey::Pubkey;

#[derive(Debug, Clone)]
pub struct RaydiumAMMPoolState {
    pub mint_0_vault: Option<Pubkey>,
    pub mint_1_vault: Option<Pubkey>,
    pub mint_0_vault_amount: Option<u64>,
    pub mint_1_vault_amount: Option<u64>,
    pub mint_0_need_take_pnl: Option<u64>,
    pub mint_1_need_take_pnl: Option<u64>,
    pub swap_fee_numerator: u64,
    pub swap_fee_denominator: u64,
}

impl RaydiumAMMPoolState {
    pub fn new(
        mint_0_vault: Option<Pubkey>,
        mint_1_vault: Option<Pubkey>,
        mint_0_vault_amount: Option<u64>,
        mint_1_vault_amount: Option<u64>,
        mint_0_need_take_pnl: Option<u64>,
        mint_1_need_take_pnl: Option<u64>,
        swap_fee_numerator: u64,
        swap_fee_denominator: u64,
    ) -> Self {
        Self {
            mint_0_vault,
            mint_1_vault,
            mint_0_vault_amount,
            mint_1_vault_amount,
            mint_0_need_take_pnl,
            mint_1_need_take_pnl,
            swap_fee_numerator,
            swap_fee_denominator,
        }
    }
}
