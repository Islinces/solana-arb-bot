//! State transition types

use crate::error::AmmError;
use solana_program::{
    account_info::AccountInfo,
    program_error::ProgramError,
    program_pack::{Pack, Sealed},
    pubkey::Pubkey,
};

use arrayref::{array_mut_ref, array_ref, array_refs, mut_array_refs};
use bytemuck::{from_bytes, from_bytes_mut, Pod, Zeroable};
use safe_transmute::{self, trivial::TriviallyTransmutable};
use std::{
    cell::{Ref, RefMut},
    mem::size_of,
};

pub trait Loadable: Pod {
    fn load_mut<'a>(account: &'a AccountInfo) -> Result<RefMut<'a, Self>, ProgramError> {
        // TODO verify if this checks for size
        Ok(RefMut::map(account.try_borrow_mut_data()?, |data| {
            from_bytes_mut(data)
        }))
    }
    fn load<'a>(account: &'a AccountInfo) -> Result<Ref<'a, Self>, ProgramError> {
        Ok(Ref::map(account.try_borrow_data()?, |data| {
            from_bytes(data)
        }))
    }

    fn load_from_bytes(data: &[u8]) -> Result<&Self, ProgramError> {
        Ok(from_bytes(data))
    }
}

macro_rules! impl_loadable {
    ($type_name:ident) => {
        unsafe impl Zeroable for $type_name {}
        unsafe impl Pod for $type_name {}
        unsafe impl TriviallyTransmutable for $type_name {}
        impl Loadable for $type_name {}
    };
}

#[repr(u64)]
pub enum AmmStatus {
    Uninitialized = 0u64,
    Initialized = 1u64,
    Disabled = 2u64,
    WithdrawOnly = 3u64,
    // pool only can add or remove liquidity, can't swap and plan orders
    LiquidityOnly = 4u64,
    // pool only can add or remove liquidity and plan orders, can't swap
    OrderBookOnly = 5u64,
    // pool only can add or remove liquidity and swap, can't plan orders
    SwapOnly = 6u64,
    // pool status after created and will auto update to SwapOnly during swap after open_time
    WaitingTrade = 7u64,
}

#[repr(C, packed)]
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct Fees {
    /// numerator of the min_separate
    pub min_separate_numerator: u64,
    /// denominator of the min_separate
    pub min_separate_denominator: u64,

    /// numerator of the fee
    pub trade_fee_numerator: u64,
    /// denominator of the fee
    /// and 'trade_fee_denominator' must be equal to 'min_separate_denominator'
    pub trade_fee_denominator: u64,

    /// numerator of the pnl
    pub pnl_numerator: u64,
    /// denominator of the pnl
    pub pnl_denominator: u64,

    /// numerator of the swap_fee
    pub swap_fee_numerator: u64,
    /// denominator of the swap_fee
    pub swap_fee_denominator: u64,
}

impl Sealed for Fees {}
impl Pack for Fees {
    const LEN: usize = 64;
    fn pack_into_slice(&self, output: &mut [u8]) {
        let output = array_mut_ref![output, 0, 64];
        let (
            min_separate_numerator,
            min_separate_denominator,
            trade_fee_numerator,
            trade_fee_denominator,
            pnl_numerator,
            pnl_denominator,
            swap_fee_numerator,
            swap_fee_denominator,
        ) = mut_array_refs![output, 8, 8, 8, 8, 8, 8, 8, 8];
        *min_separate_numerator = self.min_separate_numerator.to_le_bytes();
        *min_separate_denominator = self.min_separate_denominator.to_le_bytes();
        *trade_fee_numerator = self.trade_fee_numerator.to_le_bytes();
        *trade_fee_denominator = self.trade_fee_denominator.to_le_bytes();
        *pnl_numerator = self.pnl_numerator.to_le_bytes();
        *pnl_denominator = self.pnl_denominator.to_le_bytes();
        *swap_fee_numerator = self.swap_fee_numerator.to_le_bytes();
        *swap_fee_denominator = self.swap_fee_denominator.to_le_bytes();
    }

    fn unpack_from_slice(input: &[u8]) -> Result<Fees, ProgramError> {
        let input = array_ref![input, 0, 64];
        #[allow(clippy::ptr_offset_with_cast)]
        let (
            min_separate_numerator,
            min_separate_denominator,
            trade_fee_numerator,
            trade_fee_denominator,
            pnl_numerator,
            pnl_denominator,
            swap_fee_numerator,
            swap_fee_denominator,
        ) = array_refs![input, 8, 8, 8, 8, 8, 8, 8, 8];
        Ok(Self {
            min_separate_numerator: u64::from_le_bytes(*min_separate_numerator),
            min_separate_denominator: u64::from_le_bytes(*min_separate_denominator),
            trade_fee_numerator: u64::from_le_bytes(*trade_fee_numerator),
            trade_fee_denominator: u64::from_le_bytes(*trade_fee_denominator),
            pnl_numerator: u64::from_le_bytes(*pnl_numerator),
            pnl_denominator: u64::from_le_bytes(*pnl_denominator),
            swap_fee_numerator: u64::from_le_bytes(*swap_fee_numerator),
            swap_fee_denominator: u64::from_le_bytes(*swap_fee_denominator),
        })
    }
}

#[repr(C, packed)]
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct StateData {
    /// delay to take pnl coin
    pub need_take_pnl_coin: u64,
    /// delay to take pnl pc
    pub need_take_pnl_pc: u64,
    /// total pnl pc
    pub total_pnl_pc: u64,
    /// total pnl coin
    pub total_pnl_coin: u64,
    /// ido pool open time
    pub pool_open_time: u64,
    /// padding for future updates
    pub padding: [u64; 2],
    /// switch from orderbookonly to init
    pub orderbook_to_init_time: u64,

    /// swap coin in amount
    pub swap_coin_in_amount: u128,
    /// swap pc out amount
    pub swap_pc_out_amount: u128,
    /// charge pc as swap fee while swap pc to coin
    pub swap_acc_pc_fee: u64,

    /// swap pc in amount
    pub swap_pc_in_amount: u128,
    /// swap coin out amount
    pub swap_coin_out_amount: u128,
    /// charge coin as swap fee while swap coin to pc
    pub swap_acc_coin_fee: u64,
}

#[cfg_attr(feature = "client", derive(Debug))]
#[repr(C, packed)]
#[derive(Clone, Copy, Default, PartialEq)]
pub struct AmmInfo {
    /// Initialized status.
    pub status: u64,
    /// Nonce used in program address.
    /// The program address is created deterministically with the nonce,
    /// amm program id, and amm account pubkey.  This program address has
    /// authority over the amm's token coin account, token pc account, and pool
    /// token mint.
    pub nonce: u64,
    /// max order count
    pub order_num: u64,
    /// within this range, 5 => 5% range
    pub depth: u64,
    /// coin decimal
    pub coin_decimals: u64,
    /// pc decimal
    pub pc_decimals: u64,
    /// amm machine state
    pub state: u64,
    /// amm reset_flag
    pub reset_flag: u64,
    /// min size 1->0.000001
    pub min_size: u64,
    /// vol_max_cut_ratio numerator, sys_decimal_value as denominator
    pub vol_max_cut_ratio: u64,
    /// amount wave numerator, sys_decimal_value as denominator
    pub amount_wave: u64,
    /// coinLotSize 1 -> 0.000001
    pub coin_lot_size: u64,
    /// pcLotSize 1 -> 0.000001
    pub pc_lot_size: u64,
    /// min_cur_price: (2 * amm.order_num * amm.pc_lot_size) * max_price_multiplier
    pub min_price_multiplier: u64,
    /// max_cur_price: (2 * amm.order_num * amm.pc_lot_size) * max_price_multiplier
    pub max_price_multiplier: u64,
    /// system decimal value, used to normalize the value of coin and pc amount
    pub sys_decimal_value: u64,
    /// All fee information
    pub fees: Fees,
    /// Statistical data
    pub state_data: StateData,
    /// Coin vault
    pub coin_vault: Pubkey,
    /// Pc vault
    pub pc_vault: Pubkey,
    /// Coin vault mint
    pub coin_vault_mint: Pubkey,
    /// Pc vault mint
    pub pc_vault_mint: Pubkey,
    /// lp mint
    pub lp_mint: Pubkey,
    /// open_orders key
    pub open_orders: Pubkey,
    /// market key
    pub market: Pubkey,
    /// market program key
    pub market_program: Pubkey,
    /// target_orders key
    pub target_orders: Pubkey,
    /// padding
    pub padding1: [u64; 8],
    /// amm owner key
    pub amm_owner: Pubkey,
    /// pool lp amount
    pub lp_amount: u64,
    /// client order id
    pub client_order_id: u64,
    /// recent epoch
    pub recent_epoch: u64,
    /// padding
    pub padding2: u64,
}
impl_loadable!(AmmInfo);

impl AmmInfo {
    /// Helper function to get the more efficient packed size of the struct
    /// load_mut_checked
    #[inline]
    pub fn load_mut_checked<'a>(
        account: &'a AccountInfo,
        program_id: &Pubkey,
    ) -> Result<RefMut<'a, Self>, ProgramError> {
        if account.owner != program_id {
            return Err(AmmError::InvalidAmmAccountOwner.into());
        }
        if account.data_len() != size_of::<Self>() {
            return Err(AmmError::ExpectedAccount.into());
        }
        let data = Self::load_mut(account)?;
        if data.status == AmmStatus::Uninitialized as u64 {
            return Err(AmmError::InvalidStatus.into());
        }
        Ok(data)
    }

    /// load_checked
    #[inline]
    pub fn load_checked<'a>(
        account: &'a AccountInfo,
        program_id: &Pubkey,
    ) -> Result<Ref<'a, Self>, ProgramError> {
        if account.owner != program_id {
            return Err(AmmError::InvalidAmmAccountOwner.into());
        }
        if account.data_len() != size_of::<Self>() {
            return Err(AmmError::ExpectedAccount.into());
        }
        let data = Self::load(account)?;
        if data.status == AmmStatus::Uninitialized as u64 {
            return Err(AmmError::InvalidStatus.into());
        }
        Ok(data)
    }
}
