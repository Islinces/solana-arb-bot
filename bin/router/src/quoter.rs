use crate::account_cache::get_account_data;
use crate::dex::pump_fun::state::Pool;
use crate::dex::raydium_amm::state::AmmInfo;
use crate::dex::raydium_clmm::state::PoolState;
use crate::dex::InstructionItem;
use crate::graph::{find_mint_position, find_pool_position, EdgeIdentifier, TwoHopPath};
use crate::interface::DexType;
use rayon::iter::{IntoParallelIterator, ParallelIterator};
use solana_sdk::pubkey::Pubkey;
use std::sync::Arc;

pub fn find_best_hop_path(
    pool_id: &Pubkey,
    amount_in_mint: &Pubkey,
    amount_in: u64,
    min_profit: u64,
) -> Option<QuoteResult> {
    let pool_index = find_pool_position(pool_id)?;
    let hop_paths = crate::graph::get_graph_with_pool_index(&pool_index)?;
    let amount_in_mint_index = find_mint_position(&amount_in_mint)?;
    let (positive_hop_path, reverse_hop_path): (Vec<_>, Vec<_>) = hop_paths
        .iter()
        .filter(|hop| hop.swaped_mint() == &amount_in_mint_index)
        .cloned()
        .partition(|hop| hop.is_positive(&pool_index));
    if positive_hop_path.is_empty() && reverse_hop_path.is_empty() {
        return None;
    }
    // 正向quote，当前pool只计算一次quote，避免重复计算
    let positive_best_hop_path = positive_hop_path.first().map_or(None, |first_hop| {
        match quote(&first_hop.first, amount_in) {
            None => None,
            Some(first_amount_out) => positive_hop_path
                .iter()
                .filter_map(|hop_path| {
                    quote(&hop_path.second, first_amount_out).and_then(|second_amount_out| {
                        if has_profit(amount_in, second_amount_out, min_profit) {
                            Some((hop_path.clone(), second_amount_out - first_amount_out))
                        } else {
                            None
                        }
                    })
                })
                .max_by_key(|x| x.1),
        }
    });

    // 反向quote
    let reverse_best_hop_path = reverse_hop_path
        .into_par_iter()
        .filter_map(|hop_path| {
            quote(&hop_path.first, amount_in).and_then(|first_amount_out| {
                quote(&hop_path.second, first_amount_out).and_then(|second_amount_out| {
                    if has_profit(amount_in, second_amount_out, min_profit) {
                        Some((hop_path, second_amount_out - first_amount_out))
                    } else {
                        None
                    }
                })
            })
        })
        .max_by_key(|x| x.1);
    match (positive_best_hop_path, reverse_best_hop_path) {
        (Some((p_path, p_profit)), Some((r_path, r_profit))) => Some(if p_profit >= r_profit {
            QuoteResult::new(p_path, amount_in_mint.clone(), amount_in, p_profit)
        } else {
            QuoteResult::new(r_path, amount_in_mint.clone(), amount_in, r_profit)
        }),
        (Some((p_path, p_profit)), None) => Some(QuoteResult::new(
            p_path,
            amount_in_mint.clone(),
            amount_in,
            p_profit,
        )),
        (None, Some((r_path, r_profit))) => Some(QuoteResult::new(
            r_path,
            amount_in_mint.clone(),
            amount_in,
            r_profit,
        )),
        _ => None,
    }
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
    }
}

#[inline]
fn has_profit(amount_in: u64, amount_out: u64, min_profit: u64) -> bool {
    amount_out >= amount_in + min_profit
}

#[derive(Debug)]
pub struct QuoteResult {
    pub hop_path: Arc<TwoHopPath>,
    pub amount_in: u64,
    pub amount_in_mint: Pubkey,
    pub profit: u64,
}

impl QuoteResult {
    fn new(hop_path: Arc<TwoHopPath>, amount_in_mint: Pubkey, amount_in: u64, profit: u64) -> Self {
        Self {
            hop_path,
            amount_in_mint,
            amount_in,
            profit,
        }
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
        }
    }
}
