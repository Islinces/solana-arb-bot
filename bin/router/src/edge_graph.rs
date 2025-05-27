use crate::dex_data::DexJson;
use crate::interface::DexType;
use ahash::AHashMap;
use anyhow::anyhow;
use solana_sdk::pubkey::Pubkey;
use std::collections::hash_map::Entry;
use std::sync::Arc;
use tokio::sync::OnceCell;

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
        let dex_type = crate::interface::get_dex_type_with_program_id(&dex_json.owner).unwrap();
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
}

#[derive(Debug, Clone)]
pub struct TwoHopPath {
    first: EdgeIdentifier,
    second: EdgeIdentifier,
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
                    first: first.clone(),
                    second: second.clone(),
                })
            }
        }
    }
}

pub(crate) fn init_graph(dex_json: &[DexJson]) -> anyhow::Result<()> {
    // 初始化 mint 全局索引
    MINT_INDEX.set(Arc::new(
        dex_json
            .iter()
            .map(|v| vec![v.mint_a, v.mint_b])
            .flatten()
            .collect::<Vec<_>>(),
    ))?;
    // 初始化 pool 全局索引
    POOL_INDEX.set(Arc::new(
        dex_json.iter().map(|v| v.pool).collect::<Vec<_>>(),
    ))?;
    // 初始化所有边，用于后边构建图
    let edge_identifiers = dex_json
        .into_iter()
        .map(|v| EdgeIdentifier::new(v))
        .flatten()
        .collect::<Vec<_>>();
    // 构建图(2 hop)
    init_two_hop_graph(edge_identifiers)
}

fn init_two_hop_graph(edge_identifier: Vec<EdgeIdentifier>) -> anyhow::Result<()> {
    let two_hop_path = edge_identifier
        .iter()
        .map(|first| {
            edge_identifier
                .iter()
                .filter_map(|second| TwoHopPath::new(first, second))
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
    POOL_INDEX.get().unwrap().iter().position(|v| v == pool_id)
}

pub fn find_mint_position(mint: &Pubkey) -> Option<usize> {
    MINT_INDEX.get().unwrap().iter().position(|v| v == mint)
}

pub fn get_graph(pool_id: &Pubkey) -> Option<Arc<Vec<Arc<TwoHopPath>>>> {
    GRAPH
        .get()
        .unwrap()
        .get(&find_pool_position(pool_id)?)
        .map_or(None, |v| Some(v.clone()))
}
