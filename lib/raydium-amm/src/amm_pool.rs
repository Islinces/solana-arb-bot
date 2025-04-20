use crate::math::{CheckedCeilDiv, SwapDirection};
use crate::raydium_amm_dex::{AmmTriggerEvent, PoolUpdate};
use crate::state::AmmInfo;
use anyhow::anyhow;
use dex::interface::DexPoolInterface;
use dex::trigger::TriggerEvent;
use solana_program::pubkey::Pubkey;
use std::any::Any;
use std::sync::Arc;

#[repr(C)]
#[derive(Debug, Copy, Clone, Default)]
pub struct AmmPool {
    pub pool_id: Pubkey,
    pub owner_id: Pubkey,
    /// 金库
    pub mint_0_vault: Pubkey,
    pub mint_1_vault: Pubkey,
    pub mint_0_vault_amount: u64,
    pub mint_1_vault_amount: u64,
    /// mint
    pub mint_0: Pubkey,
    pub mint_1: Pubkey,
    /// mint 精度
    pub mint_0_decimals: u64,
    pub mint_1_decimals: u64,
    /// 交易费率
    pub swap_fee_numerator: u64,
    pub swap_fee_denominator: u64,
    /// pnl
    pub mint_0_need_take_pnl: u64,
    pub mint_1_need_take_pnl: u64,
}

impl AmmPool {}

impl From<PoolUpdate> for AmmPool {
    fn from(value: PoolUpdate) -> Self {
        Self {
            pool_id: value.pool_id,
            mint_0: value.mint_0,
            mint_1: value.mint_1,
            swap_fee_numerator: value.swap_fee_numerator,
            swap_fee_denominator: value.swap_fee_denominator,
            mint_0_need_take_pnl: value.mint_0_need_take_pnl,
            mint_1_need_take_pnl: value.mint_1_need_take_pnl,
            ..Default::default()
        }
    }
}

impl From<(AmmInfo, Pubkey, u64, u64)> for AmmPool {
    fn from(value: (AmmInfo, Pubkey, u64, u64)) -> Self {
        let amm_info = value.0;
        Self {
            pool_id: value.1,
            mint_0_vault_amount: value.2,
            mint_1_vault_amount: value.3,
            mint_0_vault: amm_info.coin_vault,
            mint_1_vault: amm_info.pc_vault,
            owner_id: amm_info.amm_owner,
            mint_0: amm_info.coin_vault_mint,
            mint_1: amm_info.pc_vault_mint,
            mint_0_decimals: amm_info.coin_decimals,
            mint_1_decimals: amm_info.pc_decimals,
            swap_fee_numerator: amm_info.fees.swap_fee_numerator,
            swap_fee_denominator: amm_info.fees.swap_fee_denominator,
            mint_0_need_take_pnl: amm_info.state_data.need_take_pnl_coin,
            mint_1_need_take_pnl: amm_info.state_data.need_take_pnl_pc,
        }
    }
}

impl DexPoolInterface for AmmPool {
    fn get_pool_id(&self) -> Pubkey {
        self.pool_id
    }

    fn get_mint_0(&self) -> Pubkey {
        self.mint_0
    }

    fn get_mint_1(&self) -> Pubkey {
        self.mint_1
    }

    fn get_mint_0_vault(&self) -> Option<Pubkey> {
        Some(self.mint_0_vault)
    }

    fn get_mint_1_vault(&self) -> Option<Pubkey> {
        Some(self.mint_1_vault)
    }

    fn as_any(&self) -> &dyn Any {
        self
    }

    fn quote(&self, amount_in: u64, amount_in_mint: Pubkey) -> Option<u64> {
        if amount_in_mint != self.mint_0 && amount_in_mint != self.mint_1 {
            return None;
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
            self.mint_0_vault_amount
                .checked_sub(self.mint_0_need_take_pnl)
                .unwrap(),
        );
        let mint_1_amount_without_pnl = u128::from(
            self.mint_1_vault_amount
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
                if amount_out >= self.mint_0_vault_amount {
                    return None;
                }
            }
            SwapDirection::Coin2PC => {
                if amount_out >= self.mint_1_vault_amount {
                    return None;
                }
            }
        }
        Some(amount_out)
    }

    fn update_data(&mut self, trigger_event: Box<dyn TriggerEvent>) -> anyhow::Result<Pubkey> {
        let changed_pool = trigger_event
            .any()
            .downcast_ref::<AmmTriggerEvent>()
            .unwrap();
        let pool_update = changed_pool.pool_update.as_ref().unwrap();
        let mint_0_vault_amount = changed_pool.mint_0_vault_update.as_ref().unwrap().amount;
        let mint_1_vault_amount = changed_pool.mint_1_vault_update.as_ref().unwrap().amount;
        let mut changed = false;
        if self.mint_0_vault_amount != mint_0_vault_amount {
            self.mint_0_vault_amount = mint_0_vault_amount;
        }
        if self.mint_1_vault_amount != mint_1_vault_amount {
            self.mint_1_vault_amount = mint_1_vault_amount;
            changed |= true;
        }
        if self.mint_0_need_take_pnl != pool_update.mint_0_need_take_pnl {
            self.mint_0_need_take_pnl = pool_update.mint_0_need_take_pnl;
            changed |= true;
        }
        if self.mint_1_need_take_pnl != pool_update.mint_1_need_take_pnl {
            self.mint_1_need_take_pnl = pool_update.mint_1_need_take_pnl;
            changed |= true;
        }
        if changed {
            Ok(self.pool_id)
        } else {
            Err(anyhow!("[{}]池子数据未发生变化", self.pool_id))
        }
    }
}
