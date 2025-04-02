use crate::math::{CheckedCeilDiv, SwapDirection};
use dex::account_write::AccountWrite;
use dex::interface::Pool;
use solana_program::pubkey::Pubkey;

#[repr(C)]
#[derive(Debug, Copy, Clone)]
pub struct AmmPool {
    pub pool_id: Pubkey,
    pub owner_id: Pubkey,
    /// 金库
    pub mint_0_vault: u64,
    pub mint_1_vault: u64,
    /// mint
    pub mint_0: Pubkey,
    pub mint_1: Pubkey,
    /// mint 精度
    pub mint_0_decimals: u8,
    pub mint_1_decimals: u8,
    /// 交易费率
    pub swap_fee_numerator: u64,
    pub swap_fee_denominator: u64,
    /// pnl
    pub mint_0_need_take_pnl: u64,
    pub mint_1_need_take_pnl: u64,
}

impl AmmPool {
    pub fn new(
        pool_id: Pubkey,
        owner_id: Pubkey,
        mint_0_vault: u64,
        mint_1_vault: u64,
        mint_0: Pubkey,
        mint_1: Pubkey,
        mint_0_decimals: u8,
        mint_1_decimals: u8,
        swap_fee_numerator: u64,
        swap_fee_denominator: u64,
        mint_0_need_take_pnl: u64,
        mint_1_need_take_pnl: u64,
    ) -> Self {
        Self {
            pool_id,
            owner_id,
            mint_0_vault,
            mint_1_vault,
            mint_0,
            mint_1,
            mint_0_decimals,
            mint_1_decimals,
            swap_fee_numerator,
            swap_fee_denominator,
            mint_0_need_take_pnl,
            mint_1_need_take_pnl,
        }
    }
}

impl Pool for AmmPool {
    fn get_pool_id(&self) -> Pubkey {
        self.pool_id
    }

    fn get_mint_0(&self) -> Pubkey {
        self.mint_0
    }

    fn get_mint_1(&self) -> Pubkey {
        self.mint_1
    }

    fn quote(&self, amount_in: u64, amount_in_mint: Pubkey) -> u64 {
        if amount_in_mint != self.mint_0 && amount_in_mint != self.mint_1 {
            return u64::MIN;
        }
        let swap_direction = match amount_in_mint == self.mint_0 {
            true => SwapDirection::Coin2PC,
            false => SwapDirection::PC2Coin,
        };
        let amount_in = u128::from(amount_in);
        let swap_fee = amount_in
            .checked_mul(u128::from(self.swap_fee_numerator))
            .unwrap()
            .checked_ceil_div(u128::from(self.swap_fee_denominator))
            .unwrap()
            .0;

        let swap_in_after_deduct_fee = amount_in.checked_sub(swap_fee).unwrap();

        let mint_0_amount_without_pnl = u128::from(
            self.mint_0_vault
                .checked_sub(self.mint_0_need_take_pnl)
                .unwrap(),
        );
        let mint_1_amount_without_pnl = u128::from(
            self.mint_1_vault
                .checked_sub(self.mint_1_need_take_pnl)
                .unwrap(),
        );
        let amount_out = match swap_direction {
            // (x + delta_x) * (y + delta_y) = x * y
            // (dst_amount + amount_in) * (src_amount - amount_out) = coin * pc
            SwapDirection::Coin2PC => mint_1_amount_without_pnl
                .checked_mul(swap_in_after_deduct_fee)
                .unwrap()
                .checked_div(
                    mint_0_amount_without_pnl
                        .checked_add(swap_in_after_deduct_fee)
                        .unwrap(),
                )
                .unwrap(),
            // (x + delta_x) * (y + delta_y) = x * y
            // (pc + amount_in) * (coin - amount_out) = coin * pc
            SwapDirection::PC2Coin => mint_0_amount_without_pnl
                .checked_mul(swap_in_after_deduct_fee)
                .unwrap()
                .checked_div(
                    mint_1_amount_without_pnl
                        .checked_add(swap_in_after_deduct_fee)
                        .unwrap(),
                )
                .unwrap(),
        };
        let amount_out = amount_out.try_into().unwrap_or_else(|_| {
            eprintln!("amount_out is too large");
            u64::MIN
        });
        match swap_direction {
            SwapDirection::PC2Coin => {
                if amount_out >= self.mint_0_vault {
                    return u64::MIN;
                }
            }
            SwapDirection::Coin2PC => {
                if amount_out >= self.mint_1_vault {
                    return u64::MIN;
                }
            }
        }
        amount_out
    }

    fn clone_box(&self) -> Box<dyn Pool> {
        Box::new(*self)
    }

    fn update_data(&self, account_write: AccountWrite) {
        todo!()
    }
}
