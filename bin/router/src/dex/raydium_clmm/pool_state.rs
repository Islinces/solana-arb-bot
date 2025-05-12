use crate::dex::common::utils::change_data_if_not_same;
use crate::dex::raydium_clmm::sdk::pool::PoolState;
use crate::dex::raydium_clmm::sdk::tick_array::{
    TickArrayState, TickState, TICK_ARRAY_SEED, TICK_ARRAY_SIZE, TICK_ARRAY_SIZE_USIZE,
};
use crate::dex::raydium_clmm::sdk::tickarray_bitmap_extension::TickArrayBitmapExtension;
use crate::interface::{DexType, GrpcMessage};
use anyhow::anyhow;
use borsh::BorshDeserialize;
use serde::Deserialize;
use solana_program::address_lookup_table::AddressLookupTableAccount;
use solana_program::pubkey::Pubkey;
use std::collections::{HashMap, HashSet, VecDeque};
use std::fmt::{Debug, Display, Formatter};
use std::time::Instant;
use tracing::info;

#[derive(Debug, Clone)]
pub struct RaydiumCLMMPoolState {
    pub amm_config: Pubkey,
    pub mint_0_vault: Pubkey,
    pub mint_1_vault: Pubkey,
    pub observation_key: Pubkey,
    pub tick_spacing: u16,
    pub trade_fee_rate: u32,
    pub liquidity: u128,
    pub sqrt_price_x64: u128,
    pub tick_current: i32,
    pub tick_array_bitmap: [u64; 16],
    pub tick_array_bitmap_extension: TickArrayBitmapExtension,
    pub tick_array_map: HashMap<Pubkey, TickArray>,
    pub tick_array_index_range: Vec<(i32, Pubkey)>,
}

impl RaydiumCLMMPoolState {
    pub fn new(
        pool_state: PoolState,
        trade_fee_rate: u32,
        bitmap_extension: TickArrayBitmapExtension,
        left_tick_arrays: Option<VecDeque<TickArrayState>>,
        right_tick_arrays: Option<VecDeque<TickArrayState>>,
    ) -> Self {
        let mut tick_array_map = HashMap::with_capacity(20);
        let mut tick_array_index = HashSet::with_capacity(20);
        let mut tick_arrays = left_tick_arrays.unwrap_or(VecDeque::new());
        tick_arrays.extend(right_tick_arrays.unwrap_or(VecDeque::new()));
        let mut pool_id = None;
        for tick_array_state in tick_arrays {
            pool_id = Some(tick_array_state.pool_id);
            let index = tick_array_state.start_tick_index;
            tick_array_index.insert(index);
            tick_array_map.insert(
                TickArrayState::key_(pool_id.as_ref().unwrap(), &index),
                TickArray::from(tick_array_state),
            );
        }
        let mut tick_array_index = tick_array_index.into_iter().collect::<Vec<_>>();
        tick_array_index.sort_unstable();
        let allowed_index_range = 10 * (pool_state.tick_spacing * 60) as i32;
        let min_index = tick_array_index
            .first()
            .unwrap()
            .checked_sub(allowed_index_range)
            .unwrap();
        let max_index = tick_array_index
            .last()
            .unwrap()
            .checked_add(allowed_index_range)
            .unwrap();
        tick_array_index.insert(0, min_index);
        tick_array_index.push(max_index);
        let tick_array_index_range = tick_array_index
            .into_iter()
            .map(|index| {
                (
                    index,
                    TickArrayState::key_(pool_id.as_ref().unwrap(), &index),
                )
            })
            .collect::<Vec<_>>();
        Self {
            amm_config: pool_state.amm_config,
            mint_0_vault: pool_state.token_vault_0,
            mint_1_vault: pool_state.token_vault_1,
            observation_key: pool_state.observation_key,
            tick_spacing: pool_state.tick_spacing,
            trade_fee_rate,
            liquidity: pool_state.liquidity,
            sqrt_price_x64: pool_state.sqrt_price_x64,
            tick_current: pool_state.tick_current,
            tick_array_bitmap: pool_state.tick_array_bitmap,
            tick_array_bitmap_extension: bitmap_extension,
            tick_array_map,
            tick_array_index_range: tick_array_index_range,
        }
    }

    pub fn get_tick_array_keys(&self, zero_to_one: bool, take_count: u8) -> Vec<Pubkey> {
        let start_index =
            TickArrayState::get_array_start_index(self.tick_current, self.tick_spacing);
        let mut tick_array_keys = self.tick_array_index_range.iter().filter(|(index, _)| {
            if zero_to_one {
                index <= &start_index
            } else {
                index >= &start_index
            }
        });
        if zero_to_one {
            tick_array_keys
                .rev()
                .take(take_count.into())
                .filter_map(|(index, tick_array_key)| {
                    self.tick_array_map
                        .contains_key(&tick_array_key)
                        .then(|| tick_array_key.clone())
                })
                .collect::<Vec<_>>()
        } else {
            tick_array_keys
                .take(take_count.into())
                .filter_map(|(index, tick_array_key)| {
                    self.tick_array_map
                        .contains_key(&tick_array_key)
                        .then(|| tick_array_key.clone())
                })
                .collect::<Vec<_>>()
        }
    }

    pub fn get_tick_arrays(&self, zero_to_one: bool, take_count: u8) -> VecDeque<TickArrayState> {
        let start_index =
            TickArrayState::get_array_start_index(self.tick_current, self.tick_spacing);
        let tick_arrays = self.tick_array_index_range.iter().filter(|(index, _)| {
            if zero_to_one {
                index <= &start_index
            } else {
                index >= &start_index
            }
        });
        if zero_to_one {
            tick_arrays
                .rev()
                .take(take_count.into())
                .filter_map(|(_, tick_array_key)| {
                    self.tick_array_map
                        .get(tick_array_key)
                        .map_or(None, |t| Some(t.clone().into()))
                })
                .collect::<VecDeque<_>>()
        } else {
            tick_arrays
                .take(take_count.into())
                .filter_map(|(_, tick_array_key)| {
                    self.tick_array_map
                        .get(tick_array_key)
                        .map_or(None, |t| Some(t.clone().into()))
                })
                .collect::<VecDeque<_>>()
        }
    }

    pub fn try_update(&mut self, grpc_message: GrpcMessage) -> anyhow::Result<()> {
        match grpc_message {
            GrpcMessage::RaydiumCLMMData(change_data, _) => {
                let mut changed =
                    change_data_if_not_same(&mut self.liquidity, change_data.liquidity);
                changed |=
                    change_data_if_not_same(&mut self.tick_current, change_data.tick_current);
                changed |=
                    change_data_if_not_same(&mut self.sqrt_price_x64, change_data.sqrt_price_x64);
                changed |= change_data_if_not_same(
                    &mut self.tick_array_bitmap,
                    change_data.tick_array_bitmap,
                );
                if changed {
                    Ok(())
                } else {
                    Err(anyhow!(""))
                }
            }
            GrpcMessage::RaydiumCLMMTickArrayData(tick_array,_) => {
                let index = tick_array.start_tick_index;
                if index.ge(&self.tick_array_index_range.first().unwrap().0)
                    && index.le(&self.tick_array_index_range.last().unwrap().0)
                {
                    match self
                        .tick_array_index_range
                        .binary_search_by_key(&index, |&(k, _)| k)
                    {
                        Ok(_) => {}
                        Err(pos) => {
                            self.tick_array_index_range
                                .insert(pos, (index, tick_array.key()));
                        }
                    }
                    self.tick_array_map.insert(tick_array.key(), tick_array);
                    Err(anyhow!(""))
                } else {
                    Err(anyhow!(
                        "TickArray index[{}]不在监控范围内",
                        tick_array.start_tick_index
                    ))
                }
            }
            _ => Err(anyhow!("")),
        }
    }
}

#[derive(Debug, Clone)]
pub struct RaydiumCLMMInstructionItem {
    pub pool_id: Pubkey,
    pub amm_config: Pubkey,
    pub mint_0: Pubkey,
    pub mint_1: Pubkey,
    pub mint_0_vault: Pubkey,
    pub mint_1_vault: Pubkey,
    pub observation_key: Pubkey,
    pub tick_arrays: Vec<Pubkey>,
    pub alt: AddressLookupTableAccount,
    pub zero_to_one: bool,
}

impl Display for RaydiumCLMMInstructionItem {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        write!(
            f,
            "{:?}: {},{:?}",
            DexType::RaydiumCLmm,
            self.pool_id,
            self.zero_to_one
        )
    }
}

#[derive(Debug, Clone, BorshDeserialize)]
pub struct TickArray {
    pub pool_id: Pubkey,
    pub start_tick_index: i32,
    pub ticks: [Tick; TICK_ARRAY_SIZE_USIZE],
    pub initialized_tick_count: u8,
}

impl TickArray {
    pub fn key(&self) -> Pubkey {
        Pubkey::find_program_address(
            &[
                TICK_ARRAY_SEED.as_bytes(),
                self.pool_id.as_ref(),
                &self.start_tick_index.to_be_bytes(),
            ],
            &crate::dex::raydium_clmm::ID,
        )
        .0
    }
}

impl From<TickArrayState> for TickArray {
    fn from(value: TickArrayState) -> Self {
        Self {
            pool_id: value.pool_id,
            start_tick_index: value.start_tick_index,
            ticks: value
                .ticks
                .into_iter()
                .map(Tick::from)
                .collect::<Vec<_>>()
                .try_into()
                .unwrap(),
            initialized_tick_count: value.initialized_tick_count,
        }
    }
}

impl Into<TickArrayState> for TickArray {
    fn into(self) -> TickArrayState {
        TickArrayState {
            pool_id: self.pool_id,
            start_tick_index: self.start_tick_index,
            ticks: self
                .ticks
                .into_iter()
                .map(|t| t.into())
                .collect::<Vec<_>>()
                .try_into()
                .unwrap(),
            initialized_tick_count: self.initialized_tick_count,
            recent_epoch: 0,
            padding: [0; 107],
        }
    }
}

#[derive(Debug, Clone, BorshDeserialize, Default)]
pub struct Tick {
    pub tick: i32,
    /// Amount of net liquidity added (subtracted) when tick is crossed from left to right (right to left)
    pub liquidity_net: i128,
    /// The total position liquidity that references this tick
    pub liquidity_gross: u128,
}

impl Tick {
    pub fn is_initialized(&self) -> bool {
        self.liquidity_gross != 0
    }
}

impl From<TickState> for Tick {
    fn from(value: TickState) -> Self {
        Self {
            tick: value.tick,
            liquidity_net: value.liquidity_net,
            liquidity_gross: value.liquidity_gross,
        }
    }
}

impl Into<TickState> for Tick {
    fn into(self) -> TickState {
        TickState {
            tick: self.tick,
            liquidity_net: self.liquidity_net,
            liquidity_gross: self.liquidity_gross,
            fee_growth_outside_0_x64: 0,
            fee_growth_outside_1_x64: 0,
            reward_growths_outside_x64: [0; 3],
            padding: [0; 13],
        }
    }
}

#[derive(Debug, Clone, Deserialize)]
pub struct PoolChangeData {
    pub pool_id: Pubkey,
    pub tick_current: i32,
    pub liquidity: u128,
    pub sqrt_price_x64: u128,
    pub tick_array_bitmap: [u64; 16],
}
