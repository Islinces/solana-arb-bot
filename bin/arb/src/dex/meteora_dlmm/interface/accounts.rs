use crate::dex::global_cache::{DynamicCache, StaticCache};
use crate::dex::meteora_dlmm::interface::typedefs::{
    Bin, StaticParameters, VariableParameters, S_PARAMETER_LEN, V_PARAMETER_LEN,
};
use crate::dex::utils::read_from;
use crate::dex::FromCache;
use parking_lot::RwLockReadGuard;
use serde::{Deserialize, Serialize};
use solana_sdk::pubkey::Pubkey;
use std::{mem, ptr};
use serde_with::serde_as;

pub const BIN_ARRAY_BITMAP_EXTENSION_ACCOUNT_DISCM: [u8; 8] = [80, 111, 124, 113, 55, 237, 18, 5];
#[repr(C)]
#[derive(Clone, Debug)]
#[cfg_attr(feature = "print_data_after_update", derive(Serialize, Deserialize))]
pub struct BinArrayBitmapExtension {
    pub lb_pair: Pubkey,
    pub positive_bin_array_bitmap: [[u64; 8]; 12],
    pub negative_bin_array_bitmap: [[u64; 8]; 12],
}

impl BinArrayBitmapExtension {
    pub fn from_slice_data(data: &[u8]) -> Self {
        unsafe { ptr::read_unaligned(data.as_ptr() as *const BinArrayBitmapExtension) }
    }
}

impl FromCache for BinArrayBitmapExtension {
    fn from_cache(
        account_key: &Pubkey,
        _static_cache: RwLockReadGuard<StaticCache>,
        dynamic_cache: &DynamicCache,
    ) -> Option<Self>
    where
        Self: Sized,
    {
        let dynamic_data = dynamic_cache.get(&account_key)?;
        let dynamic_data = dynamic_data.value().as_slice();
        Some(BinArrayBitmapExtension::from_slice_data(dynamic_data))
    }
}
pub const BIN_ARRAY_ACCOUNT_DISCM: [u8; 8] = [92, 142, 92, 220, 5, 148, 70, 181];
#[repr(C)]
#[serde_as]
#[derive(Clone, Debug)]
#[cfg_attr(feature = "print_data_after_update", derive(Serialize, Deserialize))]
pub struct BinArray {
    pub index: i64,
    pub lb_pair: Pubkey,
    #[serde_as(as = "[_; 70]")]
    pub bins: [Bin; 70],
}

impl FromCache for BinArray {
    fn from_cache(
        account_key: &Pubkey,
        _static_cache: RwLockReadGuard<StaticCache>,
        dynamic_cache: &DynamicCache,
    ) -> Option<Self>
    where
        Self: Sized,
    {
        let dynamic_data = dynamic_cache.get(&account_key)?;
        let dynamic_data = dynamic_data.value().as_slice();
        Some(BinArray::from_slice_data(dynamic_data))
    }
}

impl BinArray {
    pub fn from_slice_data(data: &[u8]) -> Self {
        unsafe {
            let index = read_from::<i64>(&data[0..8]);
            let lb_pair = read_from::<Pubkey>(&data[8..40]);
            let bins = read_from::<[Bin; 70]>(&data[40..]);
            Self {
                index,
                lb_pair,
                bins,
            }
        }
    }
}

pub const LB_PAIR_ACCOUNT_DISCM: [u8; 8] = [33, 11, 49, 98, 181, 101, 177, 13];
#[repr(C)]
#[derive(Clone, Debug)]
#[cfg_attr(feature = "print_data_after_update", derive(Serialize, Deserialize))]
pub struct LbPair {
    // static
    pub parameters: StaticParameters,
    pub pair_type: u8,
    pub bin_step: u16,
    pub status: u8,
    pub activation_type: u8,
    pub token_x_mint: Pubkey,
    pub token_y_mint: Pubkey,
    pub reserve_x: Pubkey,
    pub reserve_y: Pubkey,
    pub oracle: Pubkey,
    pub activation_point: u64,
    pub token_mint_x_program_flag: u8,
    pub token_mint_y_program_flag: u8,
    // dynamic
    pub v_parameters: VariableParameters,
    pub active_id: i32,
    pub bin_array_bitmap: [u64; 16],
}

impl FromCache for LbPair {
    fn from_cache(
        account_key: &Pubkey,
        static_cache: RwLockReadGuard<StaticCache>,
        dynamic_cache: &DynamicCache,
    ) -> Option<Self>
    where
        Self: Sized,
    {
        let static_data = static_cache.get(&account_key);
        let dynamic_data = dynamic_cache.get(&account_key)?;
        let dynamic_data = dynamic_data.value().as_slice();
        Some(LbPair::from_slice_data(static_data?, dynamic_data))
    }
}

#[test]
fn test() {
    println!("{}", mem::align_of::<VariableParameters>());
    println!("Size: {}", size_of::<VariableParameters>()); // 输出24
    println!("Align: {}", align_of::<VariableParameters>()); // 输出8

    let data = unsafe {
        VariableParameters::from_slice_data(&[
            48, 117, 0, 0, 0, 0, 0, 0, 59, 18, 0, 0, 240, 2, 60, 104, 0, 0, 0, 0, 0, 0, 0, 0,
        ])
    };
    println!("{:#?}", data);
}

impl LbPair {
    pub fn from_slice_data(static_data: &[u8], dynamic_data: &[u8]) -> Self {
        unsafe {
            let parameters = StaticParameters::from_slice_data(&static_data[0..S_PARAMETER_LEN]);
            let pair_type = read_from::<u8>(&static_data[S_PARAMETER_LEN..S_PARAMETER_LEN + 1]);
            let bin_step =
                read_from::<u16>(&static_data[S_PARAMETER_LEN + 1..S_PARAMETER_LEN + 1 + 2]);
            let status =
                read_from::<u8>(&static_data[S_PARAMETER_LEN + 1 + 2..S_PARAMETER_LEN + 1 + 2 + 1]);
            let activation_type = read_from::<u8>(
                &static_data[S_PARAMETER_LEN + 1 + 2 + 1..S_PARAMETER_LEN + 1 + 2 + 1 + 1],
            );
            let token_x_mint = read_from::<Pubkey>(
                &static_data[S_PARAMETER_LEN + 1 + 2 + 1 + 1..S_PARAMETER_LEN + 1 + 2 + 1 + 1 + 32],
            );
            let token_y_mint = read_from::<Pubkey>(
                &static_data[S_PARAMETER_LEN + 1 + 2 + 1 + 1 + 32
                    ..S_PARAMETER_LEN + 1 + 2 + 1 + 1 + 32 + 32],
            );
            let reserve_x = read_from::<Pubkey>(
                &static_data[S_PARAMETER_LEN + 1 + 2 + 1 + 1 + 32 + 32
                    ..S_PARAMETER_LEN + 1 + 2 + 1 + 1 + 32 + 32 + 32],
            );
            let reserve_y = read_from::<Pubkey>(
                &static_data[S_PARAMETER_LEN + 1 + 2 + 1 + 1 + 32 + 32 + 32
                    ..S_PARAMETER_LEN + 1 + 2 + 1 + 1 + 32 + 32 + 32 + 32],
            );
            let oracle = read_from::<Pubkey>(
                &static_data[S_PARAMETER_LEN + 1 + 2 + 1 + 1 + 32 + 32 + 32 + 32
                    ..S_PARAMETER_LEN + 1 + 2 + 1 + 1 + 32 + 32 + 32 + 32 + 32],
            );
            let activation_point = read_from::<u64>(
                &static_data[S_PARAMETER_LEN + 1 + 2 + 1 + 1 + 32 + 32 + 32 + 32 + 32
                    ..S_PARAMETER_LEN + 1 + 2 + 1 + 1 + 32 + 32 + 32 + 32 + 32 + 8],
            );

            let token_mint_x_program_flag = read_from::<u8>(
                &static_data[S_PARAMETER_LEN + 1 + 2 + 1 + 1 + 32 + 32 + 32 + 32 + 32 + 8
                    ..S_PARAMETER_LEN + 1 + 2 + 1 + 1 + 32 + 32 + 32 + 32 + 32 + 8 + 1],
            );
            let token_mint_y_program_flag = read_from::<u8>(
                &static_data[S_PARAMETER_LEN + 1 + 2 + 1 + 1 + 32 + 32 + 32 + 32 + 32 + 8 + 1
                    ..S_PARAMETER_LEN + 1 + 2 + 1 + 1 + 32 + 32 + 32 + 32 + 32 + 8 + 1 + 1],
            );
            let v_parameters =
                VariableParameters::from_slice_data(&dynamic_data[0..V_PARAMETER_LEN]);
            let active_id = read_from::<i32>(&dynamic_data[V_PARAMETER_LEN..V_PARAMETER_LEN + 4]);
            let bin_array_bitmap = read_from::<[u64; 16]>(
                &dynamic_data[V_PARAMETER_LEN + 4..V_PARAMETER_LEN + 4 + 128],
            );
            Self {
                parameters,
                pair_type,
                bin_step,
                status,
                activation_type,
                token_x_mint,
                token_y_mint,
                reserve_x,
                reserve_y,
                oracle,
                activation_point,
                token_mint_x_program_flag,
                token_mint_y_program_flag,
                v_parameters,
                active_id,
                bin_array_bitmap,
            }
        }
    }
}
