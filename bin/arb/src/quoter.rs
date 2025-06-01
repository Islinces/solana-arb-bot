use crate::account_cache::get_account_data;
use crate::dex::pump_fun::state::Pool;
use crate::dex::raydium_amm::state::AmmInfo;
use crate::dex::raydium_clmm::state::PoolState;
use crate::dex::InstructionItem;
use crate::graph::{find_mint_position, find_pool_position, EdgeIdentifier, TwoHopPath};
use crate::interface::DexType;
use rayon::iter::{IntoParallelIterator, ParallelIterator};
use solana_sdk::pubkey::Pubkey;
use std::fmt::{Display, Formatter};
use std::sync::Arc;
use tokio::task::JoinSet;

pub async fn find_best_hop_path(
    pool_id: Pubkey,
    arb_mint: Arc<Pubkey>,
    amount_in: u64,
    max_amount_in: u64,
    min_profit: u64,
) -> Option<QuoteResult> {
    let pool_index = find_pool_position(&pool_id)?;
    let hop_paths = crate::graph::get_graph_with_pool_index(pool_index)?;
    let (use_ternary_search_hop_path, normal_hop_path): (Vec<_>, Vec<_>) = hop_paths
        .iter()
        .cloned()
        .partition(|hop| hop.use_ternary_search(pool_index));
    if use_ternary_search_hop_path.is_empty() && normal_hop_path.is_empty() {
        return None;
    }
    let amount_in_mint_index = find_mint_position(arb_mint.as_ref())?;
    let mut join_set = JoinSet::new();
    join_set.spawn(async move {
        normal_quote(
            normal_hop_path,
            pool_index,
            amount_in_mint_index,
            amount_in,
            min_profit,
        )
    });
    join_set.spawn(async move {
        ternary_search_quote(use_ternary_search_hop_path, max_amount_in, min_profit)
    });
    join_set
        .join_all()
        .await
        .into_iter()
        .filter_map(|a| a)
        .max_by_key(|a| a.profit)
}

fn normal_quote(
    hop_paths: Vec<Arc<TwoHopPath>>,
    pool_index: usize,
    amount_in_mint_index: usize,
    amount_in: u64,
    min_profit: u64,
) -> Option<QuoteResult> {
    let (positive_hop_path, reverse_hop_path): (Vec<_>, Vec<_>) = hop_paths
        .iter()
        .filter(|hop| hop.swaped_mint_index() == &amount_in_mint_index)
        .cloned()
        .partition(|hop| hop.is_positive(&pool_index));
    if positive_hop_path.is_empty() && reverse_hop_path.is_empty() {
        return None;
    }
    // 正向quote，当前pool只计算一次quote，避免重复计算
    let positive_best_hop_path = positive_hop_path
        .first()
        .cloned()
        .map_or(None, |first_hop| {
            quote(&first_hop.first, amount_in).and_then(|first_amount_out| {
                positive_hop_path
                    .into_par_iter()
                    .filter_map(|hop_path| {
                        quote(&hop_path.second, first_amount_out).and_then(|second_amount_out| {
                            if has_profit(amount_in, second_amount_out, min_profit) {
                                Some((hop_path, (second_amount_out - amount_in) as i64))
                            } else {
                                None
                            }
                        })
                    })
                    .max_by_key(|x| x.1)
            })
        });
    // 反向quote
    let reverse_best_hop_path = reverse_hop_path
        .into_par_iter()
        .filter_map(|hop_path| {
            quote(&hop_path.first, amount_in).and_then(|first_amount_out| {
                quote(&hop_path.second, first_amount_out).and_then(|second_amount_out| {
                    if has_profit(amount_in, second_amount_out, min_profit) {
                        Some((hop_path, (second_amount_out - amount_in) as i64))
                    } else {
                        None
                    }
                })
            })
        })
        .max_by_key(|x| x.1);
    match (positive_best_hop_path, reverse_best_hop_path) {
        (Some((p_path, p_profit)), Some((r_path, r_profit))) => Some(if p_profit >= r_profit {
            QuoteResult::new(p_path, amount_in, p_profit)
        } else {
            QuoteResult::new(r_path, amount_in, r_profit)
        }),
        (Some((p_path, p_profit)), None) => Some(QuoteResult::new(p_path, amount_in, p_profit)),
        (None, Some((r_path, r_profit))) => Some(QuoteResult::new(r_path, amount_in, r_profit)),
        _ => None,
    }
}

fn ternary_search_quote(
    hop_paths: Vec<Arc<TwoHopPath>>,
    max_amount_in: u64,
    min_profit: u64,
) -> Option<QuoteResult> {
    hop_paths
        .into_par_iter()
        .filter_map(|hop_path| {
            find_maximize_quote_with_ternary_search(max_amount_in, |amount_in| {
                quote(&hop_path.first, amount_in).and_then(|first_amount_out| {
                    quote(&hop_path.second, first_amount_out).and_then(|second_amount_out| {
                        Some(second_amount_out as i64 - amount_in as i64)
                    })
                })
            })
            .and_then(|(best_amount_in, profit)| {
                if (profit as u64) < min_profit {
                    None
                } else {
                    Some((hop_path, best_amount_in, profit))
                }
            })
        })
        .max_by_key(|(_, _, profit)| *profit)
        .map(|(path, best_amount_in, profit)| QuoteResult::new(path, best_amount_in, profit))
}

fn quote(edge: &Arc<EdgeIdentifier>, amount_in: u64) -> Option<u64> {
    let pool_id = edge.pool_id()?;
    let dex_type = &edge.dex_type;
    match dex_type {
        DexType::RaydiumAMM => {
            let amm_info = get_account_data::<AmmInfo>(pool_id)?;
            // info!("RaydiumAMM \n{:#?}", amm_info);
            crate::dex::raydium_amm::quote::quote(amount_in, edge.swap_direction, &amm_info)
        }
        DexType::RaydiumCLMM => {
            let pool_state = get_account_data::<PoolState>(pool_id)?;
            // info!("RaydiumCLMM \n{:#?}", pool_state);
            crate::dex::raydium_clmm::quote::quote(
                amount_in,
                edge.swap_direction,
                pool_id,
                &pool_state,
            )
        }
        DexType::PumpFunAMM => {
            let pool = get_account_data::<Pool>(pool_id)?;
            // info!("PumpFunAMM \n{:#?}", pool);
            crate::dex::pump_fun::quote::quote(amount_in, edge.swap_direction, &pool)
        }
        DexType::MeteoraDLMM => {
            unimplemented!()
        }
        DexType::Token2022 => unreachable!(),
    }
}

#[inline]
fn has_profit(amount_in: u64, amount_out: u64, min_profit: u64) -> bool {
    amount_out >= amount_in + min_profit
}

/// 2hop 三元搜索最佳size
fn find_maximize_quote_with_ternary_search<Q>(max_amount_in: u64, quoter: Q) -> Option<(u64, i64)>
where
    Q: Fn(u64) -> Option<i64>,
{
    let mut left = 100_000_000u64; // 0.1 SOL
    let mut right = max_amount_in;

    let mut iterations = 0;
    let max_iterations = 50;
    let precision_threshold = 100_000_000u64; // 最小精度  0.1 SOL

    // println!("🔍 三分搜索开始：区间 = {} ~ {}", left, right);

    while right - left > precision_threshold && iterations < max_iterations {
        let mid1 = left + (right - left) / 3;
        let mid2 = right - (right - left) / 3;

        let profit1 = quoter(mid1).unwrap_or(i64::MIN);

        let profit2 = quoter(mid2).unwrap_or(i64::MIN);

        // println!("🔁 Iter {}: left={}, mid1={}, mid2={}, right={}, profit1={}, profit2={}", iterations, left, mid1, mid2, right, profit1, profit2);

        if profit1 < profit2 {
            left = mid1;
        } else {
            right = mid2;
        }

        iterations += 1;
    }

    // if iterations >= max_iterations {
    //     println!("⚠️ 达到最大迭代次数，可能未收敛");
    // } else {
    //     println!("✅ 收敛完成，共迭代 {} 次，最终区间：{} ~ {}", iterations, left, right);
    // }

    let mut best_input = 0u64;
    let mut best_profit = i64::MIN;

    // 枚举精搜 0.01 步长
    for dx in (left..=right).step_by(10_000_000) {
        let profit = quoter(dx).unwrap_or(i64::MIN);
        if profit > best_profit {
            best_profit = profit;
            best_input = dx;
        }
    }

    if best_profit > 0 {
        Some((best_input, best_profit))
    } else {
        None
    }
}

#[derive(Debug)]
pub struct QuoteResult {
    pub hop_path: Arc<TwoHopPath>,
    pub amount_in: u64,
    pub profit: i64,
}

impl QuoteResult {
    fn new(hop_path: Arc<TwoHopPath>, amount_in: u64, profit: i64) -> Self {
        Self {
            hop_path,
            amount_in,
            profit,
        }
    }

    pub fn swaped_mint(&self) -> Option<Pubkey> {
        self.hop_path.swaped_mint()
    }

    pub fn to_instructions(&self) -> Option<Vec<InstructionItem>> {
        Some(vec![
            Self::single(self.hop_path.first.as_ref())?,
            Self::single(self.hop_path.second.as_ref())?,
        ])
    }

    fn single(edge: &EdgeIdentifier) -> Option<InstructionItem> {
        match edge.dex_type {
            DexType::RaydiumAMM => crate::dex::raydium_amm::instruction::to_instruction(
                edge.pool_id()?.clone(),
                edge.swap_direction,
            ),
            DexType::RaydiumCLMM => crate::dex::raydium_clmm::instruction::to_instruction(
                edge.pool_id()?.clone(),
                edge.swap_direction,
            ),
            DexType::PumpFunAMM => crate::dex::pump_fun::instruction::to_instruction(
                edge.pool_id()?.clone(),
                edge.swap_direction,
            ),
            DexType::MeteoraDLMM => {
                unimplemented!()
            }
            DexType::Token2022 => unreachable!(),
        }
    }
}

impl Display for QuoteResult {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        let binding = Pubkey::default();
        let first_pool = self.hop_path.first.pool_id().unwrap_or(&binding);
        let second_pool = self.hop_path.second.pool_id().unwrap_or(&binding);
        let f_dex_type = &self.hop_path.first.dex_type;
        let s_dex_type = &self.hop_path.second.dex_type;
        f.write_str(&format!(
            "[{} {}] -> [{} {}], amount_in : {}, profit : {}",
            f_dex_type, first_pool, s_dex_type, second_pool, self.amount_in, self.profit
        ))
    }
}
