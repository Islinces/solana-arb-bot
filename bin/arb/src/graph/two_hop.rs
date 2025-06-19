use crate::dex::get_token_program;
use crate::dex::DexType;
use crate::dex::InstructionMaterial;
use crate::graph::{
    find_mint_by_index, find_mint_position, find_pool_position, EdgeIdentifier, HopPath,
};
use crate::metadata::MintAtaPair;
use crate::HopPathSearchResult::TwoHop;
use crate::{HopPathSearchResult, SearchResult};
use ahash::AHashMap;
use anyhow::anyhow;
use rayon::iter::IntoParallelIterator;
use rayon::iter::ParallelIterator;
use solana_sdk::pubkey::Pubkey;
use spl_associated_token_account::get_associated_token_address_with_program_id;
use std::collections::hash_map::Entry;
use std::fmt::{Display, Formatter};
use std::panic;
use std::panic::AssertUnwindSafe;
use std::sync::Arc;
use tokio::sync::OnceCell;
use tracing::error;

/// 后续针对多hop可以改成枚举，针对不同的枚举实现不同的Trigger和Quoter
static GRAPH: OnceCell<Arc<AHashMap<usize, Arc<Vec<Arc<Path>>>>>> = OnceCell::const_new();

pub fn get_graph_with_pool_index(pool_index: usize) -> Option<Arc<Vec<Arc<Path>>>> {
    GRAPH
        .get()
        .unwrap()
        .get(&pool_index)
        .map_or(None, |v| Some(v.clone()))
}

pub struct TwoHopPath;

impl HopPath for TwoHopPath {
    fn build_graph(
        &mut self,
        edge_identifiers: &[EdgeIdentifier],
        follow_mint_index: &[usize],
    ) -> anyhow::Result<()> {
        let two_hop_path = edge_identifiers
            .iter()
            .map(|first| {
                edge_identifiers
                    .iter()
                    .filter_map(|second| Path::new(first, second))
                    .filter_map(|hop_path| {
                        // 忽略掉不关注的Mint关联的路径
                        if follow_mint_index.contains(hop_path.swaped_mint_index()) {
                            Some(hop_path)
                        } else {
                            None
                        }
                    })
                    .collect::<Vec<_>>()
            })
            .flatten()
            .map(|v| Arc::new(v))
            .collect::<Vec<_>>();
        if two_hop_path.is_empty() {
            return Err(anyhow!("DexJson数据无法构建Graph"));
        }
        let mut pool_to_path = AHashMap::with_capacity(two_hop_path.len() * 2);
        let mut push_to_path = |pool_index, hop_path| match pool_to_path.entry(pool_index) {
            Entry::Vacant(entry) => {
                entry.insert(vec![hop_path]);
            }
            Entry::Occupied(mut exists) => {
                exists.get_mut().push(hop_path);
            }
        };
        for hop_path in two_hop_path {
            push_to_path(hop_path.first.pool, hop_path.clone());
            push_to_path(hop_path.second.pool, hop_path.clone());
        }
        GRAPH.set(Arc::new(
            pool_to_path
                .into_iter()
                .map(|(key, value)| (key, Arc::new(value)))
                .collect::<AHashMap<_, _>>(),
        ))?;
        Ok(())
    }

    fn find_best_hop_path(
        &self,
        pool_id: Pubkey,
        arb_mint: Arc<Pubkey>,
        amount_in: u64,
        max_amount_in: u64,
        min_profit: u64,
    ) -> Option<HopPathSearchResult> {
        let pool_index = find_pool_position(&pool_id)?;
        let hop_paths = get_graph_with_pool_index(pool_index)?;
        let mut use_ternary_search_hop_path = None;
        let mut normal_hop_path = None;
        if max_amount_in >= amount_in {
            let (temp_use_ternary_search_hop_path, temp_normal_hop_path): (Vec<_>, Vec<_>) =
                hop_paths
                    .iter()
                    .cloned()
                    .partition(|hop| hop.use_ternary_search(pool_index));
            use_ternary_search_hop_path = (!temp_use_ternary_search_hop_path.is_empty())
                .then_some(temp_use_ternary_search_hop_path);
            normal_hop_path = (!temp_normal_hop_path.is_empty()).then_some(temp_normal_hop_path);
        }

        if use_ternary_search_hop_path.is_none() && normal_hop_path.is_none() {
            return None;
        }

        let amount_in_mint_index = find_mint_position(arb_mint.as_ref())?;

        let (res1, res2) = rayon::join(
            || {
                if let Some(path) = normal_hop_path {
                    normal_quote(
                        path,
                        pool_index,
                        amount_in_mint_index,
                        amount_in,
                        min_profit,
                    )
                } else {
                    None
                }
            },
            || {
                if let Some(path) = use_ternary_search_hop_path {
                    ternary_search_quote(path, max_amount_in, min_profit)
                } else {
                    None
                }
            },
        );
        // 并行执行两种报价逻辑
        let result = panic::catch_unwind(AssertUnwindSafe(|| {
            [res1, res2]
                .into_iter()
                .flatten()
                .map(|a| HopPathSearchResult::from(TwoHop(a)))
                .max_by_key(|a| a.profit())
        }));
        if let Err(err) = result {
            error!("Rayon 线程 panic: {:?}", err);
        }
        None
    }
}

#[derive(Debug, Clone)]
struct Path {
    pub first: Arc<EdgeIdentifier>,
    pub second: Arc<EdgeIdentifier>,
}

impl Path {
    fn new(first: &EdgeIdentifier, second: &EdgeIdentifier) -> Option<Self> {
        // 同一个pool不合法
        if first.pool == second.pool {
            None
        } else {
            // 代币对相同且顺序相同
            let (first_in_mint, first_out_mint) = if first.swap_direction {
                (first.mint_0, first.mint_1)
            } else {
                (first.mint_1, first.mint_0)
            };
            let (second_in_mint, second_out_mint) = if second.swap_direction {
                (second.mint_0, second.mint_1)
            } else {
                (second.mint_1, second.mint_0)
            };
            if first_in_mint != second_out_mint || first_out_mint != second_in_mint {
                None
            } else {
                Some(Self {
                    first: Arc::new(first.clone()),
                    second: Arc::new(second.clone()),
                })
            }
        }
    }

    #[inline]
    pub fn swaped_mint_index(&self) -> &usize {
        if self.first.swap_direction {
            &self.first.mint_0
        } else {
            &self.first.mint_1
        }
    }

    #[inline]
    pub fn swaped_mint(&self) -> Option<Pubkey> {
        find_mint_by_index(if self.first.swap_direction {
            self.first.mint_0
        } else {
            self.first.mint_1
        })
    }

    #[inline]
    pub fn use_ternary_search(&self, pool_index: usize) -> bool {
        use DexType::{PumpFunAMM, RaydiumAMM};

        (self.first.pool == pool_index || self.second.pool == pool_index)
            && matches!(self.first.dex_type, PumpFunAMM | RaydiumAMM)
            && matches!(self.second.dex_type, PumpFunAMM | RaydiumAMM)
    }

    #[inline]
    pub fn is_positive(&self, pool_index: &usize) -> bool {
        &self.first.pool == pool_index
    }

    pub(crate) fn get_used_mint_ata(&self, wallet: &Pubkey) -> Vec<MintAtaPair> {
        let mint_0 = find_mint_by_index(self.first.mint_0).unwrap();
        let mint_1 = find_mint_by_index(self.first.mint_1).unwrap();
        vec![
            MintAtaPair::new(
                mint_0,
                get_associated_token_address_with_program_id(
                    wallet,
                    &mint_0,
                    &get_token_program(&mint_0),
                ),
            ),
            MintAtaPair::new(
                mint_1,
                get_associated_token_address_with_program_id(
                    wallet,
                    &mint_1,
                    &get_token_program(&mint_1),
                ),
            ),
        ]
    }
}

#[derive(Debug, Clone)]
pub struct TwoHopPathSearchResult {
    pub hop_path: Arc<Path>,
    pub amount_in: u64,
    pub profit: i64,
}

impl TwoHopPathSearchResult {
    fn new(hop_path: Arc<Path>, amount_in: u64, profit: i64) -> Self {
        Self {
            hop_path,
            amount_in,
            profit,
        }
    }
}

impl SearchResult for TwoHopPathSearchResult {
    fn profit(&self) -> i64 {
        self.profit
    }

    fn amount_in(&self) -> (u64, Pubkey) {
        (self.amount_in, self.hop_path.swaped_mint().unwrap())
    }

    fn convert_to_instruction_materials(&self) -> anyhow::Result<Vec<InstructionMaterial>> {
        Ok(vec![
            self.hop_path.first.get_instruction_material()?,
            self.hop_path.second.get_instruction_material()?,
        ])
    }

    fn information(&self) -> String {
        format!("{}", self)
    }
}

fn normal_quote(
    hop_paths: Vec<Arc<Path>>,
    pool_index: usize,
    amount_in_mint_index: usize,
    amount_in: u64,
    min_profit: u64,
) -> Option<TwoHopPathSearchResult> {
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
            first_hop
                .first
                .quote(amount_in)
                .and_then(|first_amount_out| {
                    positive_hop_path
                        .into_par_iter()
                        .filter_map(|hop_path| {
                            hop_path
                                .second
                                .quote(first_amount_out)
                                .and_then(|second_amount_out| {
                                    calculate_profit(amount_in, second_amount_out, min_profit)
                                        .and_then(|profit| Some((hop_path, profit as i64)))
                                })
                        })
                        .max_by_key(|x| x.1)
                })
        });
    // 反向quote
    let reverse_best_hop_path = reverse_hop_path
        .into_par_iter()
        .filter_map(|hop_path| {
            hop_path
                .first
                .quote(amount_in)
                .and_then(|first_amount_out| {
                    hop_path
                        .second
                        .quote(first_amount_out)
                        .and_then(|second_amount_out| {
                            calculate_profit(amount_in, second_amount_out, min_profit)
                                .and_then(|profit| Some((hop_path, profit as i64)))
                        })
                })
        })
        .max_by_key(|x| x.1);
    match (positive_best_hop_path, reverse_best_hop_path) {
        (Some((p_path, p_profit)), Some((r_path, r_profit))) => Some(if p_profit >= r_profit {
            TwoHopPathSearchResult::new(p_path, amount_in, p_profit)
        } else {
            TwoHopPathSearchResult::new(r_path, amount_in, r_profit)
        }),
        (Some((p_path, p_profit)), None) => {
            Some(TwoHopPathSearchResult::new(p_path, amount_in, p_profit))
        }
        (None, Some((r_path, r_profit))) => {
            Some(TwoHopPathSearchResult::new(r_path, amount_in, r_profit))
        }
        _ => None,
    }
}

fn ternary_search_quote(
    hop_paths: Vec<Arc<Path>>,
    max_amount_in: u64,
    min_profit: u64,
) -> Option<TwoHopPathSearchResult> {
    hop_paths
        .into_par_iter()
        .filter_map(|hop_path| {
            find_maximize_quote_with_ternary_search(max_amount_in, |amount_in| {
                hop_path
                    .first
                    .quote(amount_in)
                    .and_then(|first_amount_out| {
                        hop_path
                            .second
                            .quote(first_amount_out)
                            .and_then(|second_amount_out| {
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
        .map(|(path, best_amount_in, profit)| {
            TwoHopPathSearchResult::new(path, best_amount_in, profit)
        })
}

#[inline]
fn calculate_profit(amount_in: u64, amount_out: u64, min_profit: u64) -> Option<u64> {
    (amount_out >= amount_in + min_profit).then(|| amount_out - amount_in)
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

impl Display for TwoHopPathSearchResult {
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
