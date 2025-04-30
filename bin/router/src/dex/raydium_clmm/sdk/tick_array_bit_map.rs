///! Helper functions to get most and least significant non-zero bits
use crate::dex::raydium_clmm::sdk::big_num::U1024;
use crate::dex::raydium_clmm::sdk::error::ErrorCode;
use crate::dex::raydium_clmm::sdk::tick_array::{TickArrayState, TickState, TICK_ARRAY_SIZE};
use anchor_lang::prelude::*;

pub const TICK_ARRAY_BITMAP_SIZE: i32 = 512;

pub type TickArryBitmap = [u64; 8];

pub fn max_tick_in_tickarray_bitmap(tick_spacing: u16) -> i32 {
    i32::from(tick_spacing) * TICK_ARRAY_SIZE * TICK_ARRAY_BITMAP_SIZE
}

pub fn get_bitmap_tick_boundary(tick_array_start_index: i32, tick_spacing: u16) -> (i32, i32) {
    let ticks_in_one_bitmap: i32 = max_tick_in_tickarray_bitmap(tick_spacing);
    let mut m = tick_array_start_index.abs() / ticks_in_one_bitmap;
    if tick_array_start_index < 0 && tick_array_start_index.abs() % ticks_in_one_bitmap != 0 {
        m += 1;
    }
    let min_value: i32 = ticks_in_one_bitmap * m;
    if tick_array_start_index < 0 {
        (-min_value, -min_value + ticks_in_one_bitmap)
    } else {
        (min_value, min_value + ticks_in_one_bitmap)
    }
}

pub fn most_significant_bit(x: U1024) -> Option<u16> {
    if x.is_zero() {
        None
    } else {
        Some(u16::try_from(x.leading_zeros()).unwrap())
    }
}

pub fn least_significant_bit(x: U1024) -> Option<u16> {
    if x.is_zero() {
        None
    } else {
        Some(u16::try_from(x.trailing_zeros()).unwrap())
    }
}

/// Given a tick, calculate whether the tickarray it belongs to has been initialized.
pub fn check_current_tick_array_is_initialized(
    bit_map: U1024,
    tick_current: i32,
    tick_spacing: u16,
) -> Result<(bool, i32)> {
    // 再次判断刻度是否在池子的刻度范围内
    if TickState::check_is_out_of_boundary(tick_current) {
        return err!(ErrorCode::InvaildTickIndex);
    }
    // 单位刻度范围
    let multiplier = i32::from(tick_spacing) * TICK_ARRAY_SIZE;
    let mut compressed = tick_current / multiplier + 512;
    if tick_current < 0 && tick_current % multiplier != 0 {
        // round towards negative infinity
        compressed -= 1;
    }
    // 当前刻度范围在位图中的位置
    let bit_pos = compressed.abs();
    // set current bit
    let mask = U1024::one() << bit_pos.try_into().unwrap();
    let masked = bit_map & mask;
    // check the current bit whether initialized
    let initialized = masked != U1024::default();
    if initialized {
        return Ok((true, (compressed - 512) * multiplier));
    }
    // the current bit is not initialized
    return Ok((false, (compressed - 512) * multiplier));
}

pub fn next_initialized_tick_array_start_index(
    bit_map: U1024,
    last_tick_array_start_index: i32,
    tick_spacing: u16,
    zero_for_one: bool,
) -> (bool, i32) {
    assert!(TickArrayState::check_is_valid_start_index(
        last_tick_array_start_index,
        tick_spacing
    ));
    // 默认刻度范围的最大值
    // 默认最大1024个刻度，负数，正数
    let tick_boundary = max_tick_in_tickarray_bitmap(tick_spacing);
    // 下一个刻度开始位置
    let next_tick_array_start_index = if zero_for_one {
        last_tick_array_start_index - TickArrayState::tick_count(tick_spacing)
    } else {
        last_tick_array_start_index + TickArrayState::tick_count(tick_spacing)
    };
    // 下个刻度是否在默认刻度范围内
    // 也就是说池子刻度范围已经用完了
    if next_tick_array_start_index < -tick_boundary || next_tick_array_start_index >= tick_boundary
    {
        return (false, last_tick_array_start_index);
    }
    // 单位刻度范围值
    let multiplier = i32::from(tick_spacing) * TICK_ARRAY_SIZE;
    let mut compressed = next_tick_array_start_index / multiplier + 512;
    if next_tick_array_start_index < 0 && next_tick_array_start_index % multiplier != 0 {
        // round towards negative infinity
        compressed -= 1;
    }
    // 刻度范围位图位置
    let bit_pos = compressed.abs();

    if zero_for_one {
        // tick from upper to lower
        // find from highter bits to lower bits
        let offset_bit_map = bit_map << (1024 - bit_pos - 1).try_into().unwrap();
        // 最高有效位 例如， 8 = 1000 --> next_bit = 3
        let next_bit = most_significant_bit(offset_bit_map);
        if next_bit.is_some() {
            let next_array_start_index =
                (bit_pos - i32::from(next_bit.unwrap()) - 512) * multiplier;
            (true, next_array_start_index)
        } else {
            // 可能用完了
            // not found til to the end
            (false, -tick_boundary)
        }
    } else {
        // tick from lower to upper
        // find from lower bits to highter bits
        let offset_bit_map = bit_map >> (bit_pos).try_into().unwrap();
        let next_bit = least_significant_bit(offset_bit_map);
        if next_bit.is_some() {
            let next_array_start_index =
                (bit_pos + i32::from(next_bit.unwrap()) - 512) * multiplier;
            (true, next_array_start_index)
        } else {
            // not found til to the end
            (
                false,
                tick_boundary - TickArrayState::tick_count(tick_spacing),
            )
        }
    }
}
