use crate::account_cache::{DynamicCache, StaticCache};
use crate::dex::byte_utils::{
    read_i128, read_i32, read_pubkey, read_u128, read_u16, read_u32, read_u64,
};
use crate::dex::raydium_clmm::big_num::{U1024, U512};
use crate::dex::raydium_clmm::tick_math::{MAX_TICK, MIN_TICK};
use crate::dex::FromCache;
use crate::interface::DexType;
use crate::{require, require_gt};
use anyhow::anyhow;
use borsh::BorshDeserialize;
use parking_lot::RwLockReadGuard;
use solana_sdk::pubkey::Pubkey;
use std::ptr;

pub const AMM_CONFIG_SEED: &str = "amm_config";
/// Seed to derive account address and signature
pub const POOL_SEED: &str = "pool";
pub const POOL_VAULT_SEED: &str = "pool_vault";
pub const POOL_REWARD_VAULT_SEED: &str = "pool_reward_vault";
pub const POOL_TICK_ARRAY_BITMAP_SEED: &str = "pool_tick_array_bitmap_extension";
// Number of rewards Token
pub const REWARD_NUM: usize = 3;

pub const FEE_RATE_DENOMINATOR_VALUE: u32 = 1_000_000;
/// Holds the current owner of the factory
#[derive(Default, Debug)]
pub struct AmmConfig {
    /// The trade fee, denominated in hundredths of a bip (10^-6)
    pub trade_fee_rate: u32,
}

impl FromCache for AmmConfig {
    fn from_cache(
        account_key: &Pubkey,
        static_cache: RwLockReadGuard<StaticCache>,
        _dynamic_cache: &DynamicCache,
    ) -> Option<Self>
    where
        Self: Sized,
    {
        let static_data = static_cache.get(account_key)?;
        unsafe {
            Some(Self {
                trade_fee_rate: read_u32(&static_data),
            })
        }
    }
}

pub fn pda_amm_config_key(index: u16) -> Pubkey {
    Pubkey::find_program_address(
        &[AMM_CONFIG_SEED.as_bytes(), &index.to_be_bytes()],
        DexType::RaydiumCLMM.get_ref_program_id(),
    )
    .0
}

#[repr(C, packed)]
#[derive(Default, Debug)]
pub struct PoolState {
    // ================= static data ====================
    pub amm_config: Pubkey,
    /// Token pair of the pool, where token_mint_0 address < token_mint_1 address
    pub token_mint_0: Pubkey,
    pub token_mint_1: Pubkey,
    /// Token pair vault
    pub token_vault_0: Pubkey,
    pub token_vault_1: Pubkey,
    /// observation account key
    pub observation_key: Pubkey,
    /// The minimum number of ticks between initialized ticks
    pub tick_spacing: u16,
    // ================= dynamic data ====================
    /// The currently in range liquidity available to the pool.
    pub liquidity: u128,
    /// The current price of the pool as a sqrt(token_1/token_0) Q64.64 value
    pub sqrt_price_x64: u128,
    /// The current tick of the pool, i.e. according to the last tick transition that was run.
    pub tick_current: i32,
    /// Packed initialized tick array state
    pub tick_array_bitmap: [u64; 16],
}

impl FromCache for PoolState {
    fn from_cache(
        account_key: &Pubkey,
        static_cache: RwLockReadGuard<StaticCache>,
        dynamic_cache: &DynamicCache,
    ) -> Option<Self>
    where
        Self: Sized,
    {
        let pool_static_data = static_cache.get(&account_key)?;
        let pool_dynamic_data = dynamic_cache.get(&account_key)?;
        Some(PoolState::from_slice_data(
            pool_static_data,
            pool_dynamic_data.as_slice(),
        ))
    }
}

impl PoolState {
    pub fn from_slice_data(static_data: &[u8], dynamic_data: &[u8]) -> Self {
        unsafe {
            let amm_config = read_pubkey(&static_data[0..32]);
            let token_mint_0 = read_pubkey(&static_data[32..64]);
            let token_mint_1 = read_pubkey(&static_data[64..96]);
            let token_vault_0 = read_pubkey(&static_data[96..128]);
            let token_vault_1 = read_pubkey(&static_data[128..160]);
            let observation_key = read_pubkey(&static_data[160..192]);
            let tick_spacing = read_u16(&static_data[192..194]);
            let liquidity = read_u128(&dynamic_data[0..16]);
            let sqrt_price_x64 = read_u128(&dynamic_data[16..32]);
            let tick_current = read_i32(&dynamic_data[32..36]);
            let mut tick_array_bitmap = ptr::read_unaligned(
                dynamic_data[36..16 + 16 + 4 + 8 * 16].as_ptr() as *const [u64; 16],
            );
            // let mut tick_array_bitmap = [0; 16];
            // for (index, data) in dynamic_data[36..16 + 16 + 4 + 8 * 16]
            //     .chunks(8)
            //     .into_iter()
            //     .enumerate()
            // {
            //     tick_array_bitmap[index] = ptr::read_unaligned(data.as_ptr() as *const u64);
            // }
            Self {
                amm_config,
                token_mint_0,
                token_mint_1,
                token_vault_0,
                token_vault_1,
                observation_key,
                tick_spacing,
                liquidity,
                sqrt_price_x64,
                tick_current,
                tick_array_bitmap,
            }
        }
    }

    pub fn get_first_initialized_tick_array(
        &self,
        tickarray_bitmap_extension: &Option<TickArrayBitmapExtension>,
        zero_for_one: bool,
    ) -> anyhow::Result<(bool, i32)> {
        let (is_initialized, start_index) =
            if self.is_overflow_default_tickarray_bitmap(vec![self.tick_current]) {
                tickarray_bitmap_extension
                    .as_ref()
                    .unwrap()
                    .check_tick_array_is_initialized(
                        TickArrayState::get_array_start_index(self.tick_current, self.tick_spacing),
                        self.tick_spacing,
                    )?
            } else {
                check_current_tick_array_is_initialized(
                    U1024(self.tick_array_bitmap),
                    self.tick_current,
                    self.tick_spacing.into(),
                )?
            };
        if is_initialized {
            return Ok((true, start_index));
        }
        let next_start_index = self.next_initialized_tick_array_start_index(
            tickarray_bitmap_extension,
            TickArrayState::get_array_start_index(self.tick_current, self.tick_spacing),
            zero_for_one,
        )?;
        require!(
            next_start_index.is_some(),
            "Insufficient liquidity for this direction"
        );
        Ok((false, next_start_index.unwrap()))
    }

    pub fn next_initialized_tick_array_start_index(
        &self,
        tickarray_bitmap_extension: &Option<TickArrayBitmapExtension>,
        mut last_tick_array_start_index: i32,
        zero_for_one: bool,
    ) -> anyhow::Result<Option<i32>> {
        last_tick_array_start_index =
            TickArrayState::get_array_start_index(last_tick_array_start_index, self.tick_spacing);

        loop {
            let (is_found, start_index) = next_initialized_tick_array_start_index(
                U1024(self.tick_array_bitmap),
                last_tick_array_start_index,
                self.tick_spacing,
                zero_for_one,
            );
            if is_found {
                return Ok(Some(start_index));
            }
            last_tick_array_start_index = start_index;

            if tickarray_bitmap_extension.is_none() {
                return Err(anyhow!("Missing tickarray bitmap extension account"));
            }
            let (is_found, start_index) = tickarray_bitmap_extension
                .as_ref()
                .unwrap()
                .next_initialized_tick_array_from_one_bitmap(
                    last_tick_array_start_index,
                    self.tick_spacing,
                    zero_for_one,
                )?;
            if is_found {
                return Ok(Some(start_index));
            }
            last_tick_array_start_index = start_index;
            if last_tick_array_start_index < MIN_TICK || last_tick_array_start_index > MAX_TICK {
                return Ok(None);
            }
        }
    }

    pub fn is_overflow_default_tickarray_bitmap(&self, tick_indexs: Vec<i32>) -> bool {
        let (min_tick_array_start_index_boundary, max_tick_array_index_boundary) =
            self.tick_array_start_index_range();
        for tick_index in tick_indexs {
            let tick_array_start_index =
                TickArrayState::get_array_start_index(tick_index, self.tick_spacing);
            if tick_array_start_index >= max_tick_array_index_boundary
                || tick_array_start_index < min_tick_array_start_index_boundary
            {
                return true;
            }
        }
        false
    }

    /// the range of tick array start index that default tickarray bitmap can represent
    /// if tick_spacing = 1, the result range is [-30720, 30720)
    pub fn tick_array_start_index_range(&self) -> (i32, i32) {
        // the range of ticks that default tickarrary can represent
        let mut max_tick_boundary = max_tick_in_tickarray_bitmap(self.tick_spacing);
        let mut min_tick_boundary = -max_tick_boundary;
        if max_tick_boundary > MAX_TICK {
            max_tick_boundary = TickArrayState::get_array_start_index(MAX_TICK, self.tick_spacing);
            // find the next tick array start index
            max_tick_boundary = max_tick_boundary + TickArrayState::tick_count(self.tick_spacing);
        }
        if min_tick_boundary < MIN_TICK {
            min_tick_boundary = TickArrayState::get_array_start_index(MIN_TICK, self.tick_spacing);
        }
        (min_tick_boundary, max_tick_boundary)
    }
}

pub const TICK_ARRAY_SEED: &str = "tick_array";
pub const TICK_ARRAY_SIZE_USIZE: usize = 60;
pub const TICK_ARRAY_SIZE: i32 = 60;

#[repr(C, packed)]
#[derive(Debug, Clone)]
pub struct TickArrayState {
    pub pool_id: Pubkey,
    pub start_tick_index: i32,
    pub ticks: [TickState; TICK_ARRAY_SIZE_USIZE],
}

impl FromCache for TickArrayState {
    fn from_cache(
        account_key: &Pubkey,
        _static_cache: RwLockReadGuard<StaticCache>,
        dynamic_cache: &DynamicCache,
    ) -> Option<Self>
    where
        Self: Sized,
    {
        let dynamic_slice_data = dynamic_cache.get(account_key)?;
        unsafe {
            Some(ptr::read_unaligned(
                dynamic_slice_data.as_slice().as_ptr() as *const TickArrayState
            ))
        }
    }
}

impl TickArrayState {
    pub fn key(&self) -> Pubkey {
        Pubkey::find_program_address(
            &[
                TICK_ARRAY_SEED.as_bytes(),
                self.pool_id.as_ref(),
                &self.start_tick_index.to_be_bytes(),
            ],
            DexType::RaydiumCLMM.get_ref_program_id(),
        )
        .0
    }

    /// Base on swap directioin, return the first initialized tick in the tick array.
    pub fn first_initialized_tick(&mut self, zero_for_one: bool) -> anyhow::Result<&mut TickState> {
        if zero_for_one {
            let mut i = TICK_ARRAY_SIZE - 1;
            while i >= 0 {
                if self.ticks[i as usize].is_initialized() {
                    return Ok(self.ticks.get_mut(i as usize).unwrap());
                }
                i = i - 1;
            }
        } else {
            let mut i = 0;
            while i < TICK_ARRAY_SIZE_USIZE {
                if self.ticks[i].is_initialized() {
                    return Ok(self.ticks.get_mut(i).unwrap());
                }
                i = i + 1;
            }
        }
        Err(anyhow!("Invaild tick array account"))
    }

    /// Get next initialized tick in tick array, `current_tick_index` can be any tick index, in other words, `current_tick_index` not exactly a point in the tickarray,
    /// and current_tick_index % tick_spacing maybe not equal zero.
    /// If price move to left tick <= current_tick_index, or to right tick > current_tick_index
    pub fn next_initialized_tick(
        &mut self,
        current_tick_index: i32,
        tick_spacing: u16,
        zero_for_one: bool,
    ) -> anyhow::Result<Option<&mut TickState>> {
        let current_tick_array_start_index =
            TickArrayState::get_array_start_index(current_tick_index, tick_spacing);
        if current_tick_array_start_index != self.start_tick_index {
            return Ok(None);
        }
        let mut offset_in_array =
            (current_tick_index - self.start_tick_index) / i32::from(tick_spacing);

        if zero_for_one {
            while offset_in_array >= 0 {
                if self.ticks[offset_in_array as usize].is_initialized() {
                    return Ok(self.ticks.get_mut(offset_in_array as usize));
                }
                offset_in_array = offset_in_array - 1;
            }
        } else {
            offset_in_array = offset_in_array + 1;
            while offset_in_array < TICK_ARRAY_SIZE {
                if self.ticks[offset_in_array as usize].is_initialized() {
                    return Ok(self.ticks.get_mut(offset_in_array as usize));
                }
                offset_in_array = offset_in_array + 1;
            }
        }
        Ok(None)
    }

    /// Base on swap directioin, return the next tick array start index.
    pub fn next_tick_arrary_start_index(&self, tick_spacing: u16, zero_for_one: bool) -> i32 {
        let ticks_in_array = TICK_ARRAY_SIZE * i32::from(tick_spacing);
        if zero_for_one {
            self.start_tick_index - ticks_in_array
        } else {
            self.start_tick_index + ticks_in_array
        }
    }

    /// Input an arbitrary tick_index, output the start_index of the tick_array it sits on
    pub fn get_array_start_index(tick_index: i32, tick_spacing: u16) -> i32 {
        let ticks_in_array = TickArrayState::tick_count(tick_spacing);
        let mut start = tick_index / ticks_in_array;
        if tick_index < 0 && tick_index % ticks_in_array != 0 {
            start = start - 1
        }
        start * ticks_in_array
    }

    pub fn check_is_valid_start_index(tick_index: i32, tick_spacing: u16) -> bool {
        if TickState::check_is_out_of_boundary(tick_index) {
            if tick_index > MAX_TICK {
                return false;
            };
            let min_start_index = TickArrayState::get_array_start_index(MIN_TICK, tick_spacing);
            return tick_index == min_start_index;
        }
        tick_index % TickArrayState::tick_count(tick_spacing) == 0
    }

    pub fn tick_count(tick_spacing: u16) -> i32 {
        TICK_ARRAY_SIZE * i32::from(tick_spacing)
    }
}

impl Default for TickArrayState {
    #[inline]
    fn default() -> TickArrayState {
        TickArrayState {
            pool_id: Pubkey::default(),
            ticks: [TickState::default(); TICK_ARRAY_SIZE_USIZE],
            start_tick_index: 0,
        }
    }
}

#[repr(C, packed)]
#[derive(Default, Debug, Clone, Copy, BorshDeserialize)]
pub struct TickState {
    pub tick: i32,
    /// Amount of net liquidity added (subtracted) when tick is crossed from left to right (right to left)
    pub liquidity_net: i128,
    /// The total position liquidity that references this tick
    pub liquidity_gross: u128,
}

impl TickState {
    pub fn from_slice_data(slice_data: &[u8]) -> Self {
        unsafe {
            let tick = read_i32(&slice_data[0..4]);
            let liquidity_net = read_i128(&slice_data[4..20]);
            let liquidity_gross = read_u128(&slice_data[20..36]);
            Self {
                tick,
                liquidity_net,
                liquidity_gross,
            }
        }
    }

    pub fn is_initialized(self) -> bool {
        self.liquidity_gross != 0
    }

    /// Common checks for a valid tick input.
    /// A tick is valid if it lies within tick boundaries
    pub fn check_is_out_of_boundary(tick: i32) -> bool {
        tick < MIN_TICK || tick > MAX_TICK
    }
}

pub const TICK_ARRAY_BITMAP_SIZE: i32 = 512;

pub type TickArrayBitmap = [u64; 8];

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
) -> anyhow::Result<(bool, i32)> {
    if TickState::check_is_out_of_boundary(tick_current) {
        return Err(anyhow!("Tick out of range"));
    }
    let multiplier = i32::from(tick_spacing) * TICK_ARRAY_SIZE;
    let mut compressed = tick_current / multiplier + 512;
    if tick_current < 0 && tick_current % multiplier != 0 {
        // round towards negative infinity
        compressed -= 1;
    }
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
    Ok((false, (compressed - 512) * multiplier))
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
    let tick_boundary = max_tick_in_tickarray_bitmap(tick_spacing);
    let next_tick_array_start_index = if zero_for_one {
        last_tick_array_start_index - TickArrayState::tick_count(tick_spacing)
    } else {
        last_tick_array_start_index + TickArrayState::tick_count(tick_spacing)
    };
    if next_tick_array_start_index < -tick_boundary || next_tick_array_start_index >= tick_boundary
    {
        return (false, last_tick_array_start_index);
    }
    let multiplier = i32::from(tick_spacing) * TICK_ARRAY_SIZE;
    let mut compressed = next_tick_array_start_index / multiplier + 512;
    if next_tick_array_start_index < 0 && next_tick_array_start_index % multiplier != 0 {
        // round towards negative infinity
        compressed -= 1;
    }
    let bit_pos = compressed.abs();

    if zero_for_one {
        // tick from upper to lower
        // find from highter bits to lower bits
        let offset_bit_map = bit_map << (1024 - bit_pos - 1).try_into().unwrap();
        let next_bit = most_significant_bit(offset_bit_map);
        if next_bit.is_some() {
            let next_array_start_index =
                (bit_pos - i32::from(next_bit.unwrap()) - 512) * multiplier;
            (true, next_array_start_index)
        } else {
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

pub fn pda_bit_map_extension_key(pool_id: &Pubkey) -> Pubkey {
    Pubkey::find_program_address(
        &[POOL_TICK_ARRAY_BITMAP_SEED.as_bytes(), pool_id.as_ref()],
        DexType::RaydiumCLMM.get_ref_program_id(),
    )
    .0
}
const EXTENSION_TICKARRAY_BITMAP_SIZE: usize = 14;
#[repr(C, packed)]
#[derive(Debug, Clone)]
pub struct TickArrayBitmapExtension {
    pub pool_id: Pubkey,
    /// Packed initialized tick array state for start_tick_index is positive
    pub positive_tick_array_bitmap: [[u64; 8]; EXTENSION_TICKARRAY_BITMAP_SIZE],
    /// Packed initialized tick array state for start_tick_index is negitive
    pub negative_tick_array_bitmap: [[u64; 8]; EXTENSION_TICKARRAY_BITMAP_SIZE],
}

impl FromCache for TickArrayBitmapExtension {
    fn from_cache(
        account_key: &Pubkey,
        _static_cache: RwLockReadGuard<StaticCache>,
        dynamic_cache: &DynamicCache,
    ) -> Option<Self>
    where
        Self: Sized,
    {
        let dynamic_slice_data = dynamic_cache.get(&account_key)?;
        Some(TickArrayBitmapExtension::from_slice_data(
            dynamic_slice_data.as_slice(),
        ))
    }
}

impl TickArrayBitmapExtension {
    pub fn from_slice_data(dynamic_data: &[u8]) -> Self {
        unsafe {
            let pool_id = read_pubkey(&dynamic_data[0..32]);
            let mut positive_tick_array_bitmap = [[0; 8]; EXTENSION_TICKARRAY_BITMAP_SIZE];
            let mut negative_tick_array_bitmap = [[0; 8]; EXTENSION_TICKARRAY_BITMAP_SIZE];
            let tick_array_bitmap_data = &dynamic_data[32..];
            for (index, data) in tick_array_bitmap_data
                .chunks(tick_array_bitmap_data.len() / 2)
                .enumerate()
            {
                let mut bitmap_array = [[0; 8]; EXTENSION_TICKARRAY_BITMAP_SIZE];
                for (i, d1) in data.chunks(8 * 8).enumerate() {
                    let mut bitmap = [0; 8];
                    for (j, d2) in d1.chunks(8).enumerate() {
                        bitmap[j] = read_u64(&d2[0..8]);
                    }
                    bitmap_array[i] = bitmap;
                }
                if index == 0 {
                    positive_tick_array_bitmap = bitmap_array;
                } else {
                    negative_tick_array_bitmap = bitmap_array;
                }
            }
            Self {
                pool_id,
                positive_tick_array_bitmap,
                negative_tick_array_bitmap,
            }
        }
    }

    fn get_bitmap_offset(tick_index: i32, tick_spacing: u16) -> anyhow::Result<usize> {
        require!(
            TickArrayState::check_is_valid_start_index(tick_index, tick_spacing),
            "Tick out of range"
        );
        Self::check_extension_boundary(tick_index, tick_spacing)?;
        let ticks_in_one_bitmap = max_tick_in_tickarray_bitmap(tick_spacing);
        let mut offset = tick_index.abs() / ticks_in_one_bitmap - 1;
        if tick_index < 0 && tick_index.abs() % ticks_in_one_bitmap == 0 {
            offset -= 1;
        }
        Ok(offset as usize)
    }

    /// According to the given tick, calculate its corresponding tickarray and then find the bitmap it belongs to.
    fn get_bitmap(
        &self,
        tick_index: i32,
        tick_spacing: u16,
    ) -> anyhow::Result<(usize, TickArrayBitmap)> {
        let offset = Self::get_bitmap_offset(tick_index, tick_spacing)?;
        if tick_index < 0 {
            Ok((offset, self.negative_tick_array_bitmap[offset]))
        } else {
            Ok((offset, self.positive_tick_array_bitmap[offset]))
        }
    }

    /// Check if the tick in tick array bitmap extension
    pub fn check_extension_boundary(tick_index: i32, tick_spacing: u16) -> anyhow::Result<()> {
        let positive_tick_boundary = max_tick_in_tickarray_bitmap(tick_spacing);
        let negative_tick_boundary = -positive_tick_boundary;
        require_gt!(MAX_TICK, positive_tick_boundary);
        require_gt!(negative_tick_boundary, MIN_TICK);
        if tick_index >= negative_tick_boundary && tick_index < positive_tick_boundary {
            return Err(anyhow!("Invaild tick array boundary"));
        }
        Ok(())
    }

    /// Check if the tick array is initialized
    pub fn check_tick_array_is_initialized(
        &self,
        tick_array_start_index: i32,
        tick_spacing: u16,
    ) -> anyhow::Result<(bool, i32)> {
        // 当前刻度范围对应的位图数据
        let (_, tickarray_bitmap) = self.get_bitmap(tick_array_start_index, tick_spacing)?;
        // 当前刻度开始位置在单位刻度范围内的位置
        let tick_array_offset_in_bitmap =
            Self::tick_array_offset_in_bitmap(tick_array_start_index, tick_spacing);

        if U512(tickarray_bitmap).bit(tick_array_offset_in_bitmap as usize) {
            return Ok((true, tick_array_start_index));
        }
        Ok((false, tick_array_start_index))
    }

    /// Search for the first initialized bit in bitmap according to the direction, if found return ture and the tick array start index,
    /// if not, return false and tick boundary index
    pub fn next_initialized_tick_array_from_one_bitmap(
        &self,
        last_tick_array_start_index: i32,
        tick_spacing: u16,
        zero_for_one: bool,
    ) -> anyhow::Result<(bool, i32)> {
        let multiplier = TickArrayState::tick_count(tick_spacing);
        let next_tick_array_start_index = if zero_for_one {
            last_tick_array_start_index - multiplier
        } else {
            last_tick_array_start_index + multiplier
        };
        let min_tick_array_start_index =
            TickArrayState::get_array_start_index(MIN_TICK, tick_spacing);
        let max_tick_array_start_index =
            TickArrayState::get_array_start_index(MAX_TICK, tick_spacing);
        if next_tick_array_start_index < min_tick_array_start_index
            || next_tick_array_start_index > max_tick_array_start_index
        {
            return Ok((false, next_tick_array_start_index));
        }

        let (_, tickarray_bitmap) = self.get_bitmap(next_tick_array_start_index, tick_spacing)?;

        Ok(Self::next_initialized_tick_array_in_bitmap(
            tickarray_bitmap,
            next_tick_array_start_index,
            tick_spacing,
            zero_for_one,
        ))
    }

    pub fn next_initialized_tick_array_in_bitmap(
        tickarray_bitmap: TickArrayBitmap,
        next_tick_array_start_index: i32,
        tick_spacing: u16,
        zero_for_one: bool,
    ) -> (bool, i32) {
        let (bitmap_min_tick_boundary, bitmap_max_tick_boundary) =
            get_bitmap_tick_boundary(next_tick_array_start_index, tick_spacing);

        let tick_array_offset_in_bitmap =
            Self::tick_array_offset_in_bitmap(next_tick_array_start_index, tick_spacing);
        if zero_for_one {
            // tick from upper to lower
            // find from highter bits to lower bits
            let offset_bit_map = U512(tickarray_bitmap)
                << (TICK_ARRAY_BITMAP_SIZE - 1 - tick_array_offset_in_bitmap);

            let next_bit = if offset_bit_map.is_zero() {
                None
            } else {
                Some(u16::try_from(offset_bit_map.leading_zeros()).unwrap())
            };

            if next_bit.is_some() {
                let next_array_start_index = next_tick_array_start_index
                    - i32::from(next_bit.unwrap()) * TickArrayState::tick_count(tick_spacing);
                return (true, next_array_start_index);
            } else {
                // not found til to the end
                return (false, bitmap_min_tick_boundary);
            }
        } else {
            // tick from lower to upper
            // find from lower bits to highter bits
            let offset_bit_map = U512(tickarray_bitmap) >> tick_array_offset_in_bitmap;

            let next_bit = if offset_bit_map.is_zero() {
                None
            } else {
                Some(u16::try_from(offset_bit_map.trailing_zeros()).unwrap())
            };
            if next_bit.is_some() {
                let next_array_start_index = next_tick_array_start_index
                    + i32::from(next_bit.unwrap()) * TickArrayState::tick_count(tick_spacing);
                (true, next_array_start_index)
            } else {
                // not found til to the end
                (
                    false,
                    bitmap_max_tick_boundary - TickArrayState::tick_count(tick_spacing),
                )
            }
        }
    }

    pub fn tick_array_offset_in_bitmap(tick_array_start_index: i32, tick_spacing: u16) -> i32 {
        let m = tick_array_start_index.abs() % max_tick_in_tickarray_bitmap(tick_spacing);
        let mut tick_array_offset_in_bitmap = m / TickArrayState::tick_count(tick_spacing);
        if tick_array_start_index < 0 && m != 0 {
            tick_array_offset_in_bitmap = TICK_ARRAY_BITMAP_SIZE - tick_array_offset_in_bitmap;
        }
        tick_array_offset_in_bitmap
    }
}
