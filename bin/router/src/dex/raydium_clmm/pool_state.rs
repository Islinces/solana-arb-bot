use crate::cache::Pool;
use crate::dex::common::utils::change_data_if_not_same;
use crate::dex::raydium_clmm::sdk::pool::PoolState;
use crate::dex::raydium_clmm::sdk::tick_array::{
    TickArrayState, TickState, TICK_ARRAY_SEED, TICK_ARRAY_SIZE_USIZE,
};
use crate::dex::raydium_clmm::sdk::tickarray_bitmap_extension::TickArrayBitmapExtension;
use crate::interface::GrpcAccountUpdateType::TickArrayState as TickArrayStateUpdateType;
use crate::interface::{DexType, GrpcAccountUpdateType, GrpcMessage, SubscribeKey};
use anyhow::anyhow;
use borsh::BorshDeserialize;
use solana_sdk::address_lookup_table::AddressLookupTableAccount;
use solana_sdk::pubkey::Pubkey;
use std::collections::{HashMap, HashSet, VecDeque};
use std::fmt::{Debug, Display, Formatter};
use tracing::info;
use yellowstone_grpc_proto::geyser::subscribe_request_filter_accounts_filter::Filter;
use yellowstone_grpc_proto::geyser::{
    subscribe_request_filter_accounts_filter_memcmp, CommitmentLevel, SubscribeRequest,
    SubscribeRequestAccountsDataSlice, SubscribeRequestFilterAccounts,
    SubscribeRequestFilterAccountsFilter, SubscribeRequestFilterAccountsFilterMemcmp,
};

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
    pub tick_array_map: HashMap<Pubkey, TickArrayMonitorData>,
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
                TickArrayMonitorData::from(tick_array_state),
            );
        }
        let mut tick_array_index = tick_array_index.into_iter().collect::<Vec<_>>();
        tick_array_index.sort_unstable();
        // let allowed_index_range = 10 * (pool_state.tick_spacing * 60) as i32;
        // let min_index = tick_array_index
        //     .first()
        //     .unwrap()
        //     .checked_sub(allowed_index_range)
        //     .unwrap();
        // let max_index = tick_array_index
        //     .last()
        //     .unwrap()
        //     .checked_add(allowed_index_range)
        //     .unwrap();
        // tick_array_index.insert(0, min_index);
        // tick_array_index.push(max_index);
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
            tick_array_index_range,
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
            GrpcMessage::RaydiumClmmMonitorData(change_data, _, _, ..) => {
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
            GrpcMessage::RaydiumClmmTickArrayMonitorData(tick_array, _) => {
                let index = tick_array.start_tick_index;
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
pub struct TickArrayMonitorData {
    pub pool_id: Pubkey,
    pub start_tick_index: i32,
    pub ticks: [Tick; TICK_ARRAY_SIZE_USIZE],
    pub initialized_tick_count: u8,
}

impl TickArrayMonitorData {
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

    pub fn subscribe_request(pools: &[Pool]) -> (SubscribeKey, SubscribeRequest) {
        let mut tick_arrays_subscribe_accounts = HashMap::new();
        for (index, pool) in pools.iter().enumerate() {
            tick_arrays_subscribe_accounts.insert(
                format!("{:?}:{:?}:{:?}", DexType::RaydiumCLmm, TickArrayStateUpdateType, index),
                SubscribeRequestFilterAccounts {
                    nonempty_txn_signature:None,
                    account: vec![],
                    owner: vec![DexType::RaydiumCLmm.get_program_id().to_string()],
                    filters: vec![
                        SubscribeRequestFilterAccountsFilter {
                            filter: Some(Filter::Datasize(10240)),
                        },
                        SubscribeRequestFilterAccountsFilter {
                            filter: Some(
                                Filter::Memcmp(SubscribeRequestFilterAccountsFilterMemcmp{
                                    offset: 8,
                                    data: Some(
                                        subscribe_request_filter_accounts_filter_memcmp::Data::Bytes(
                                            pool.pool_id.to_bytes().to_vec(),
                                        ),
                                    ),
                                }),
                            ),
                        },
                    ],
                },
            );
        }
        let mut tick_arrays_data_slice = vec![
            // pool_id
            SubscribeRequestAccountsDataSlice {
                offset: 8,
                length: 32,
            },
            // start_tick_index
            SubscribeRequestAccountsDataSlice {
                offset: 40,
                length: 4,
            },
        ];
        let mut start_index = 40 + 4;
        // ticks 60个
        for _ in 0..TICK_ARRAY_SIZE_USIZE {
            // tick
            tick_arrays_data_slice.push(SubscribeRequestAccountsDataSlice {
                offset: start_index,
                length: 4,
            });
            start_index += 4;
            // liquidity_net
            tick_arrays_data_slice.push(SubscribeRequestAccountsDataSlice {
                offset: start_index,
                length: 16,
            });
            start_index += 16;
            // liquidity_gross
            tick_arrays_data_slice.push(SubscribeRequestAccountsDataSlice {
                offset: start_index,
                length: 16,
            });
            start_index += 16;
            // fee_growth_outside_0_x64
            start_index += 16;
            // fee_growth_outside_1_x64
            start_index += 16;
            // reward_growths_outside_x64
            start_index += 16 * 3;
            // padding
            start_index += 13 * 4;
        }
        tick_arrays_data_slice.push(
            // initialized_tick_count
            SubscribeRequestAccountsDataSlice {
                offset: 10124,
                length: 1,
            },
        );
        let tick_arrays_subscribe_request = SubscribeRequest {
            accounts: tick_arrays_subscribe_accounts,
            commitment: Some(CommitmentLevel::Processed).map(|x| x as i32),
            accounts_data_slice: tick_arrays_data_slice,
            ..Default::default()
        };
        (
            (DexType::RaydiumCLmm, GrpcAccountUpdateType::TickArrayState),
            tick_arrays_subscribe_request,
        )
    }
}

impl From<TickArrayState> for TickArrayMonitorData {
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

impl Into<TickArrayState> for TickArrayMonitorData {
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

#[derive(Debug, Clone, BorshDeserialize)]
pub struct PoolMonitorData {
    pub liquidity: u128,
    pub sqrt_price_x64: u128,
    pub tick_current: i32,
    pub tick_array_bitmap: [u64; 16],
}

impl PoolMonitorData {
    pub fn subscribe_request(pools: &[Pool]) -> (SubscribeKey, SubscribeRequest) {
        let mut subscribe_pool_accounts = HashMap::new();
        subscribe_pool_accounts.insert(
            format!("{:?}", DexType::RaydiumCLmm),
            SubscribeRequestFilterAccounts {
                account: pools
                    .iter()
                    .map(|pool| pool.pool_id.to_string())
                    .collect::<Vec<_>>(),
                ..Default::default()
            },
        );
        let pool_request = SubscribeRequest {
            accounts: subscribe_pool_accounts,
            commitment: Some(CommitmentLevel::Processed).map(|x| x as i32),
            accounts_data_slice: vec![
                // liquidity
                SubscribeRequestAccountsDataSlice {
                    offset: 237,
                    length: 16,
                },
                // sqrt_price_x64
                SubscribeRequestAccountsDataSlice {
                    offset: 253,
                    length: 16,
                },
                // tick_current
                SubscribeRequestAccountsDataSlice {
                    offset: 269,
                    length: 4,
                },
                // tick_array_bitmap
                SubscribeRequestAccountsDataSlice {
                    offset: 904,
                    length: 128,
                },
            ],
            ..Default::default()
        };
        (
            (DexType::RaydiumCLmm, GrpcAccountUpdateType::Pool),
            pool_request,
        )
    }
}
