use crate::dex::{get_dex_type_with_program_id, DexType};
use crate::dex_data::DexJson;
use crate::global_cache::get_token_program;
use ahash::AHashMap;
use anyhow::anyhow;
use solana_sdk::pubkey::Pubkey;
use spl_associated_token_account::get_associated_token_address_with_program_id;
use std::collections::hash_map::Entry;
use std::sync::Arc;
use tokio::sync::OnceCell;
use tracing::info;

/// 后续针对多hop可以改成枚举，针对不同的枚举实现不同的Trigger和Quoter
static GRAPH: OnceCell<Arc<AHashMap<usize, Arc<Vec<Arc<TwoHopPath>>>>>> = OnceCell::const_new();
/// Mint 全局索引，节省内存
static MINT_INDEX: OnceCell<Arc<Vec<Pubkey>>> = OnceCell::const_new();
/// Pool 全局索引，节省内存
static POOL_INDEX: OnceCell<Arc<Vec<Pubkey>>> = OnceCell::const_new();

#[derive(Debug, Clone)]
pub struct EdgeIdentifier {
    pub dex_type: DexType,
    pub pool: usize,
    pub mint_0: usize,
    pub mint_1: usize,
    // true : mint_0 -> mint_1
    // false : mint_1 -> mint_0
    pub swap_direction: bool,
}

impl EdgeIdentifier {
    fn new(dex_json: &DexJson) -> Vec<Self> {
        let dex_type = get_dex_type_with_program_id(&dex_json.owner).unwrap();
        let pool = find_pool_position(&dex_json.pool).unwrap();
        let mint_0 = find_mint_position(&dex_json.mint_a).unwrap();
        let mint_1 = find_mint_position(&dex_json.mint_b).unwrap();
        vec![
            Self {
                dex_type: dex_type.clone(),
                pool,
                mint_0,
                mint_1,
                swap_direction: true,
            },
            Self {
                dex_type,
                pool,
                mint_0,
                mint_1,
                swap_direction: false,
            },
        ]
    }

    #[inline]
    pub fn pool_id(&self) -> Option<&Pubkey> {
        POOL_INDEX
            .get()?
            .get(self.pool)
            .map_or(None, |pool| Some(pool))
    }
}

#[derive(Debug, Clone)]
pub struct TwoHopPath {
    pub first: Arc<EdgeIdentifier>,
    pub second: Arc<EdgeIdentifier>,
}

impl TwoHopPath {
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

    pub fn get_relate_mint_ata(&self, wallet: &Pubkey) -> Vec<(Pubkey, Pubkey)> {
        let mint_0 = find_mint_by_index(self.first.mint_0).unwrap();
        let mint_1 = find_mint_by_index(self.first.mint_1).unwrap();
        vec![
            (
                get_associated_token_address_with_program_id(
                    wallet,
                    &mint_0,
                    &get_token_program(&mint_0),
                ),
                mint_0,
            ),
            (
                get_associated_token_address_with_program_id(
                    wallet,
                    &mint_1,
                    &get_token_program(&mint_1),
                ),
                mint_1,
            ),
        ]
    }
}

pub(crate) fn init_graph(dex_json: &[DexJson], follow_mints: &[Pubkey]) -> anyhow::Result<()> {
    info!("初始化Graph...");
    // 初始化 pool 全局索引
    POOL_INDEX.set(Arc::new(
        dex_json.iter().map(|v| v.pool).collect::<Vec<_>>(),
    ))?;
    // 初始化 mint 全局索引
    MINT_INDEX.set(Arc::new(
        dex_json
            .iter()
            .map(|v| vec![v.mint_a, v.mint_b])
            .flatten()
            .collect::<Vec<_>>(),
    ))?;
    // 关注的Mint的index
    let follow_mint_index = follow_mints
        .iter()
        .filter_map(|v| find_mint_position(v))
        .collect::<Vec<_>>();
    // 初始化所有边，用于后边构建图
    let edge_identifiers = dex_json
        .into_iter()
        .map(|v| EdgeIdentifier::new(v))
        .flatten()
        .collect::<Vec<_>>();
    // 构建图(2 hop)
    let graph = init_two_hop_graph(edge_identifiers, follow_mint_index.as_slice());
    info!("初始化Graph结束");
    graph
}

fn init_two_hop_graph(
    edge_identifier: Vec<EdgeIdentifier>,
    follow_mints: &[usize],
) -> anyhow::Result<()> {
    let two_hop_path = edge_identifier
        .iter()
        .map(|first| {
            edge_identifier
                .iter()
                .filter_map(|second| TwoHopPath::new(first, second))
                .filter_map(|hop_path| {
                    // 忽略掉不关注的Mint关联的路径
                    if follow_mints.contains(hop_path.swaped_mint_index()) {
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

pub fn find_pool_position(pool_id: &Pubkey) -> Option<usize> {
    POOL_INDEX.get()?.iter().position(|v| v == pool_id)
}

pub fn find_mint_position(mint: &Pubkey) -> Option<usize> {
    MINT_INDEX.get()?.iter().position(|v| v == mint)
}

pub fn find_mint_by_index(index: usize) -> Option<Pubkey> {
    MINT_INDEX.get()?.get(index).cloned()
}

pub fn get_graph_with_pool_index(pool_index: usize) -> Option<Arc<Vec<Arc<TwoHopPath>>>> {
    GRAPH
        .get()
        .unwrap()
        .get(&pool_index)
        .map_or(None, |v| Some(v.clone()))
}
