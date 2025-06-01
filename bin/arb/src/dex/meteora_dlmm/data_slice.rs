use crate::data_slice::{retain_intervals_unsafe, SliceType};
use crate::interface::AccountType;
use anyhow::anyhow;
use tokio::sync::OnceCell;

// ========================= dynamic data 账户订阅的数据切片 =========================
// dlmm pool
static DYNAMIC_POOL_SLICE: OnceCell<([(usize, usize); 6], usize)> = OnceCell::const_new();
static DYNAMIC_BIN_ARRAY_SLICE: OnceCell<(Vec<(usize, usize)>, usize)> = OnceCell::const_new();
static DYNAMIC_BIN_ARRAY_BITMAP_EXTENSION_SLICE: OnceCell<([(usize, usize); 1], usize)> =
    OnceCell::const_new();
// ========================= static data 账户未订阅的数据切片 =========================
// dlmm pool
static STATIC_POOL_SLICE: OnceCell<([(usize, usize); 20], usize)> = OnceCell::const_new();

pub fn slice_data(
    account_type: AccountType,
    data: Vec<u8>,
    slice_type: SliceType,
) -> anyhow::Result<Vec<u8>> {
    match slice_type {
        SliceType::Subscribed => match account_type {
            AccountType::Pool => Ok(retain_intervals_unsafe(
                data,
                &DYNAMIC_POOL_SLICE.get().unwrap().0,
                DYNAMIC_POOL_SLICE.get().unwrap().1,
            )),
            AccountType::BinArray => Ok(retain_intervals_unsafe(
                data,
                &DYNAMIC_BIN_ARRAY_SLICE.get().unwrap().0.as_slice(),
                DYNAMIC_BIN_ARRAY_SLICE.get().unwrap().1,
            )),
            AccountType::BinArrayBitmap => Ok(retain_intervals_unsafe(
                data,
                &DYNAMIC_BIN_ARRAY_BITMAP_EXTENSION_SLICE
                    .get()
                    .unwrap()
                    .0
                    .as_slice(),
                DYNAMIC_BIN_ARRAY_BITMAP_EXTENSION_SLICE.get().unwrap().1,
            )),
            _ => Err(anyhow!("")),
        },
        SliceType::Unsubscribed => match account_type {
            AccountType::Pool => Ok(retain_intervals_unsafe(
                data,
                &STATIC_POOL_SLICE.get().unwrap().0,
                STATIC_POOL_SLICE.get().unwrap().1,
            )),
            _ => Err(anyhow!("")),
        },
    }
}

pub fn get_slice_size(
    account_type: AccountType,
    slice_type: SliceType,
) -> anyhow::Result<Option<usize>> {
    match slice_type {
        SliceType::Subscribed => match account_type {
            AccountType::Pool => Ok(Some(DYNAMIC_POOL_SLICE.get().unwrap().1)),
            AccountType::BinArray => Ok(Some(DYNAMIC_BIN_ARRAY_SLICE.get().unwrap().1)),
            AccountType::BinArrayBitmap => Ok(Some(
                DYNAMIC_BIN_ARRAY_BITMAP_EXTENSION_SLICE.get().unwrap().1,
            )),
            _ => Err(anyhow!("DexType和AccountType不匹配")),
        },
        SliceType::Unsubscribed => match account_type {
            AccountType::Pool => Ok(Some(STATIC_POOL_SLICE.get().unwrap().1)),
            _ => Err(anyhow!("DexType和AccountType不匹配")),
        },
    }
}

pub fn init_data_slice() {
    STATIC_POOL_SLICE
        .set({
            (
                [
                    // parameters.base_factor 1
                    (8, 8 + 2),
                    // parameters.filter_period 1
                    (10, 10 + 2),
                    // parameters.decay_period 1
                    (12, 12 + 2),
                    // parameters.reduction_factor 1
                    (14, 14 + 2),
                    // parameters.variable_fee_control 1
                    (16, 16 + 4),
                    // parameters.max_volatility_accumulator 1
                    (20, 20 + 4),
                    // parameters.min_bin_id
                    // (24, 24 + 4),
                    // parameters.max_bin_id
                    // (28, 28 + 4),
                    // parameters.protocol_share 1
                    (32, 32 + 2),
                    // parameters.base_fee_power_factor 1
                    (34, 34 + 1),
                    // parameters.padding
                    // (35,35+5),
                    // v_parameters
                    // (40,40+32),
                    // bump_seed
                    // (72,72_1),
                    // bin_step_seed
                    // (73,73+2),
                    // pair_type 1
                    (75, 75 + 1),
                    // active_id
                    // (76,76+4),
                    // bin_step 1
                    (80, 80 + 2),
                    // status 1
                    (82, 82 + 1),
                    // require_base_factor_seed
                    // (83, 83 + 1),
                    // base_factor_seed
                    // (84, 84 + 2),
                    // activation_type 1
                    (86, 86 + 1),
                    // creator_pool_on_off_control
                    // (87, 87 + 1),
                    // token_x_mint
                    (88, 88 + 32),
                    // token_y_mint
                    (120, 120 + 32),
                    // reserve_x
                    (152, 152 + 32),
                    // reserve_y
                    (184, 184 + 32),
                    // protocol_fee.amount_x
                    // (216, 216 + 8),
                    // protocol_fee.amount_y
                    // (224, 224 + 8),
                    // padding1
                    // (232,232+32),
                    // reward_infos
                    // (264,264+144*2)
                    // oracle
                    (552, 552 + 32),
                    // bin_array_bitmap
                    // (584, 584 + 128),
                    // last_updated_at
                    // (712, 712 + 8),
                    // padding2
                    // (720,720+32),
                    // pre_activation_swap_address
                    // (752, 752 + 32),
                    // base_key
                    // (784, 784 + 32),
                    // activation_point
                    (816, 816 + 8),
                    // pre_activation_duration
                    // (824, 824 + 8),
                    // padding3
                    // (832,832+8),
                    // padding4
                    // (840,840+8)
                    // creator
                    // (848, 848 + 32),
                    // token_mint_x_program_flag
                    (880, 880 + 1),
                    // token_mint_y_program_flag
                    (881, 881 + 1),
                    // reserved
                    // (882, 882 + 22),
                ],
                186+8,
            )
        })
        .unwrap();
    DYNAMIC_POOL_SLICE
        .set({
            (
                [
                    // v_parameters.volatility_accumulator 1
                    (40, 40 + 4),
                    // v_parameters.volatility_reference 1
                    (44, 44 + 4),
                    // v_parameters.index_reference 1
                    (48, 48 + 4),
                    // v_parameters.last_update_timestamp 1
                    (56, 56 + 8),
                    // active_id 1
                    (76, 76 + 4),
                    // bin_array_bitmap 1
                    (584, 584 + 128),
                ],
                4 + 4 + 4 + 8 + 4 + 128,
            )
        })
        .unwrap();
    DYNAMIC_BIN_ARRAY_SLICE
        .set({
            let mut slice = Vec::new();
            let mut total_len = 0;
            let mut start_index = 8;
            // index
            slice.push((start_index, start_index + 8));
            start_index += 8;
            total_len += 8;
            // version
            // slice.push((index, index + 1));
            start_index += 1;
            // total_len += 1;
            // padding
            // slice.push((index,index+7));
            start_index += 7;
            // total_len += 7;
            // lb_pair
            slice.push((start_index, start_index + 32));
            start_index += 32;
            total_len += 32;
            // bins
            for _ in 0..70 {
                // amount_x
                slice.push((start_index, start_index + 8));
                start_index += 8;
                total_len += 8;
                // amount_y
                slice.push((start_index, start_index + 8));
                start_index += 8;
                total_len += 8;
                // price
                slice.push((start_index, start_index + 16));
                start_index += 16;
                total_len += 16;
                // liquidity_supply,
                // slice.push((start_index, start_index + 16));
                start_index += 16;
                // total_len += 16;
                // reward_per_token_stored
                // slice.push((start_index, start_index + 16 * 2));
                start_index += 16 * 2;
                // total_len += 16 * 2;
                // fee_amount_x_per_token_stored
                // slice.push((start_index, start_index + 16));
                start_index += 16;
                // total_len += 16;
                // fee_amount_y_per_token_stored
                // slice.push((start_index, start_index + 16));
                start_index += 16;
                // total_len += 16;
                // amount_x_in
                // slice.push((start_index, start_index + 16));
                start_index += 16;
                // total_len += 16;
                // amount_y_in
                // slice.push((start_index, start_index + 16));
                start_index += 16;
                // total_len += 16;
            }
            (slice, total_len)
        })
        .unwrap();
    DYNAMIC_BIN_ARRAY_BITMAP_EXTENSION_SLICE
        .set({ ([(8, 8 + 1568)], 1568) })
        .unwrap()
}
