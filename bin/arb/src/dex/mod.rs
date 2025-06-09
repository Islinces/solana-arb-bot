use crate::account_cache::{DynamicCache, StaticCache};
use crate::interface1::DexType;
use parking_lot::RwLockReadGuard;
use solana_sdk::instruction::AccountMeta;
use solana_sdk::message::AddressLookupTableAccount;
use solana_sdk::pubkey::Pubkey;

mod amm_math;
pub mod byte_utils;
pub mod meteora_dlmm;
pub mod pump_fun;
pub mod raydium_amm;
pub mod raydium_clmm;
pub mod orca_whirlpools;

pub trait FromCache {
    fn from_cache(
        account_key: &Pubkey,
        static_cache: RwLockReadGuard<StaticCache>,
        dynamic_cache: &DynamicCache,
    ) -> Option<Self>
    where
        Self: Sized;
}

pub struct InstructionItem {
    pub dex_type: DexType,
    pub swap_direction: bool,
    pub account_meta: Vec<AccountMeta>,
    pub alts: Vec<AddressLookupTableAccount>,
}

impl InstructionItem {
    pub fn new(
        dex_type: DexType,
        swap_direction: bool,
        account_meta: Vec<AccountMeta>,
        alts: Vec<AddressLookupTableAccount>,
    ) -> Self {
        Self {
            dex_type,
            swap_direction,
            account_meta,
            alts,
        }
    }
}

pub(crate) fn get_transfer_fee(mint: &Pubkey, epoch: u64, pre_fee_amount: u64) -> u64 {
    if let Some(fee_config) = crate::account_cache::get_token2022_data(mint) {
        fee_config
            .calculate_epoch_fee(epoch, pre_fee_amount)
            .unwrap()
    } else {
        0
    }
}
