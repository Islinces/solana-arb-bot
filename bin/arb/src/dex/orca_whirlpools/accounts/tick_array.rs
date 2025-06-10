use crate::dex::byte_utils::read_from;
use crate::dex::orca_whirlpools::{get_tick_array_start_tick_index, Whirlpool, WHIRLPOOL_ID};
use crate::dex::FromCache;
use crate::global_cache::{DynamicCache, StaticCache};
use parking_lot::RwLockReadGuard;
use solana_sdk::program_error::ProgramError;
use solana_sdk::pubkey::Pubkey;

/// The number of ticks in a tick array.
pub const TICK_ARRAY_SIZE: usize = 88;

/// Pools with tick spacing above this threshold are considered full range only.
/// This means the program rejects any non-full range positions in these pools.
pub const FULL_RANGE_ONLY_TICK_SPACING_THRESHOLD: u16 = 32768; // 2^15

/// The minimum tick index.
pub const MIN_TICK_INDEX: i32 = -443636;

/// The maximum tick index.
pub const MAX_TICK_INDEX: i32 = 443636;

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct TickArray {
    // 8,4
    pub start_tick_index: i32,
    // 12,113*88
    pub ticks: [Tick; 88],
    // 9956,32
    pub whirlpool: Pubkey,
}

impl FromCache for TickArray {
    fn from_cache(
        account_key: &Pubkey,
        _static_cache: RwLockReadGuard<StaticCache>,
        dynamic_cache: &DynamicCache,
    ) -> Option<Self>
    where
        Self: Sized,
    {
        let dynamic_data = dynamic_cache.get(account_key)?;
        let dynamic_data = dynamic_data.value().as_slice();
        TickArray::from_slice_data(dynamic_data).map_or(None, |tick_array| Some(tick_array))
    }
}

impl TickArray {
    pub fn from_slice_data(dynamic_data: &[u8]) -> anyhow::Result<Self> {
        unsafe {
            let start_tick_index = read_from::<i32>(&dynamic_data[0..4]);
            let mut ticks = [Tick::default(); 88];
            for (index, tick_byte) in dynamic_data[4..2908].chunks(33).enumerate() {
                let initialized = read_from::<bool>(&tick_byte[0..1]);
                let liquidity_net = read_from::<i128>(&tick_byte[1..17]);
                let liquidity_gross = read_from::<u128>(&tick_byte[17..33]);
                ticks[index] = Tick {
                    initialized,
                    liquidity_net,
                    liquidity_gross,
                };
            }
            let whirlpool = read_from::<Pubkey>(&dynamic_data[2904..2936]);
            Ok(Self {
                start_tick_index,
                ticks,
                whirlpool,
            })
        }
    }

    #[inline(always)]
    pub fn from_bytes(data: &[u8]) -> Result<Self, std::io::Error> {
        let mut data = data;
        unsafe {
            let start_tick_index = read_from::<i32>(&data[8..12]);
            let mut ticks = [Tick::default(); 88];
            for (index, tick_byte) in data[12..9956].chunks(113).enumerate() {
                let initialized = read_from::<bool>(&tick_byte[0..1]);
                let liquidity_net = read_from::<i128>(&tick_byte[1..17]);
                let liquidity_gross = read_from::<u128>(&tick_byte[17..33]);
                ticks[index] = Tick {
                    initialized,
                    liquidity_net,
                    liquidity_gross,
                };
            }
            let whirlpool = read_from::<Pubkey>(&data[9956..9988]);
            Ok(Self {
                start_tick_index,
                ticks,
                whirlpool,
            })
        }
    }
}

pub fn get_tick_array_address(
    whirlpool: &Pubkey,
    start_tick_index: i32,
) -> Result<(Pubkey, u8), ProgramError> {
    let start_tick_index_str = start_tick_index.to_string();
    let seeds = &[
        b"tick_array",
        whirlpool.as_ref(),
        start_tick_index_str.as_bytes(),
    ];

    Pubkey::try_find_program_address(seeds, &WHIRLPOOL_ID).ok_or(ProgramError::InvalidSeeds)
}

pub(crate) fn get_tick_array_keys(
    whirlpool_address: Pubkey,
    whirlpool: &Whirlpool,
    load_count: u8,
    swap_direction: bool,
) -> Result<Vec<Pubkey>, ProgramError> {
    let tick_array_start_index =
        get_tick_array_start_tick_index(whirlpool.tick_current_index, whirlpool.tick_spacing);
    let offset = whirlpool.tick_spacing as i32 * TICK_ARRAY_SIZE as i32;
    let mut tick_array_indexes = Vec::with_capacity(load_count as usize);
    tick_array_indexes.push(tick_array_start_index);
    for index in 0..load_count - 1 {
        if swap_direction {
            tick_array_indexes.push(tick_array_start_index - offset * (index as i32 + 1));
        } else {
            tick_array_indexes.push(tick_array_start_index + offset * (index as i32 + 1));
        }
    }
    tick_array_indexes
        .iter()
        .map(|&x| get_tick_array_address(&whirlpool_address, x).map(|y| y.0))
        .collect::<Result<Vec<Pubkey>, _>>()
}

#[derive(Clone, Copy, Debug, Eq, PartialEq, Default)]
pub struct Tick {
    // 1
    pub initialized: bool,
    // 16
    pub liquidity_net: i128,
    // 16
    pub liquidity_gross: u128,
    // 16
    // fee_a
    // 16
    // fee_b
    // 16*3
    // reward
}

#[derive(Copy, Clone, Debug, PartialEq, Eq, Default)]
pub struct TickFacade {
    pub initialized: bool,
    pub liquidity_net: i128,
    pub liquidity_gross: u128,
}

#[derive(Copy, Clone, Debug, PartialEq, Eq)]
pub struct TickArrayFacade {
    pub start_tick_index: i32,
    pub ticks: [TickFacade; TICK_ARRAY_SIZE],
}

impl From<TickArray> for TickArrayFacade {
    fn from(val: TickArray) -> Self {
        TickArrayFacade {
            start_tick_index: val.start_tick_index,
            ticks: val.ticks.map(|tick| tick.into()),
        }
    }
}

impl From<Tick> for TickFacade {
    fn from(val: Tick) -> Self {
        TickFacade {
            liquidity_net: val.liquidity_net,
            liquidity_gross: val.liquidity_gross,
            initialized: val.initialized,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum TickArrays {
    One(TickArrayFacade),
    Two(TickArrayFacade, TickArrayFacade),
    Three(TickArrayFacade, TickArrayFacade, TickArrayFacade),
    Four(
        TickArrayFacade,
        TickArrayFacade,
        TickArrayFacade,
        TickArrayFacade,
    ),
    Five(
        TickArrayFacade,
        TickArrayFacade,
        TickArrayFacade,
        TickArrayFacade,
        TickArrayFacade,
    ),
    Six(
        TickArrayFacade,
        TickArrayFacade,
        TickArrayFacade,
        TickArrayFacade,
        TickArrayFacade,
        TickArrayFacade,
    ),
}

impl From<TickArrays> for [Option<TickArrayFacade>; 6] {
    fn from(val: TickArrays) -> Self {
        match val {
            TickArrays::One(tick_array) => [Some(tick_array), None, None, None, None, None],
            TickArrays::Two(tick_array_1, tick_array_2) => [
                Some(tick_array_1),
                Some(tick_array_2),
                None,
                None,
                None,
                None,
            ],
            TickArrays::Three(tick_array_1, tick_array_2, tick_array_3) => [
                Some(tick_array_1),
                Some(tick_array_2),
                Some(tick_array_3),
                None,
                None,
                None,
            ],
            TickArrays::Four(tick_array_1, tick_array_2, tick_array_3, tick_array_4) => [
                Some(tick_array_1),
                Some(tick_array_2),
                Some(tick_array_3),
                Some(tick_array_4),
                None,
                None,
            ],
            TickArrays::Five(
                tick_array_1,
                tick_array_2,
                tick_array_3,
                tick_array_4,
                tick_array_5,
            ) => [
                Some(tick_array_1),
                Some(tick_array_2),
                Some(tick_array_3),
                Some(tick_array_4),
                Some(tick_array_5),
                None,
            ],
            TickArrays::Six(
                tick_array_1,
                tick_array_2,
                tick_array_3,
                tick_array_4,
                tick_array_5,
                tick_array_6,
            ) => [
                Some(tick_array_1),
                Some(tick_array_2),
                Some(tick_array_3),
                Some(tick_array_4),
                Some(tick_array_5),
                Some(tick_array_6),
            ],
        }
    }
}

impl From<TickArrayFacade> for TickArrays {
    fn from(val: TickArrayFacade) -> Self {
        TickArrays::One(val)
    }
}

impl From<[TickArrayFacade; 1]> for TickArrays {
    fn from(val: [TickArrayFacade; 1]) -> Self {
        TickArrays::One(val[0])
    }
}

impl From<[TickArrayFacade; 2]> for TickArrays {
    fn from(val: [TickArrayFacade; 2]) -> Self {
        TickArrays::Two(val[0], val[1])
    }
}

impl From<[TickArrayFacade; 3]> for TickArrays {
    fn from(val: [TickArrayFacade; 3]) -> Self {
        TickArrays::Three(val[0], val[1], val[2])
    }
}

impl From<[TickArrayFacade; 4]> for TickArrays {
    fn from(val: [TickArrayFacade; 4]) -> Self {
        TickArrays::Four(val[0], val[1], val[2], val[3])
    }
}

impl From<[TickArrayFacade; 5]> for TickArrays {
    fn from(val: [TickArrayFacade; 5]) -> Self {
        TickArrays::Five(val[0], val[1], val[2], val[3], val[4])
    }
}

impl From<[TickArrayFacade; 6]> for TickArrays {
    fn from(val: [TickArrayFacade; 6]) -> Self {
        TickArrays::Six(val[0], val[1], val[2], val[3], val[4], val[5])
    }
}
