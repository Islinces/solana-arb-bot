use crate::core::InstructionMaterialConverter;
use crate::core::Quoter;
use crate::dex::{get_dex_type_with_program_id, DexType};
use crate::dex_data::DexJson;
use crate::{
    get_instruction_builder, get_quoter_type, InstructionMaterial, QuoteResult, TwoHopPath,
    TwoHopPathSearchResult,
};
use anyhow::anyhow;
use async_trait::async_trait;
use enum_dispatch::enum_dispatch;
use parking_lot::RwLock;
use solana_sdk::pubkey::Pubkey;
use std::fmt::Display;
use std::sync::Arc;
use tokio::sync::OnceCell;
use tracing::info;

/// Mint 全局索引，节省内存
pub(crate) static MINT_INDEX: OnceCell<Arc<Vec<Pubkey>>> = OnceCell::const_new();
/// Pool 全局索引，节省内存
pub(crate) static POOL_INDEX: OnceCell<Arc<Vec<Pubkey>>> = OnceCell::const_new();

#[enum_dispatch(HopPathTypes)]
pub(crate) trait HopPath: Send + Sync {
    fn build_graph(
        &mut self,
        edge_identifiers: &[EdgeIdentifier],
        follow_mint_index: &[usize],
    ) -> anyhow::Result<()>;

    fn find_best_hop_path(
        &self,
        pool_id: Pubkey,
        arb_mint: Arc<Pubkey>,
        amount_in: u64,
        max_amount_in: u64,
        min_profit: u64,
    ) -> Option<HopPathSearchResult>;
}

#[enum_dispatch]
pub enum HopPathTypes {
    TwoHop(TwoHopPath),
}

#[enum_dispatch(HopPathSearchResult)]
pub trait SearchResult {
    fn profit(&self) -> i64;

    fn amount_in(&self) -> (u64, Pubkey);

    fn convert_to_instruction_materials(&self) -> anyhow::Result<Vec<InstructionMaterial>>;

    fn information(&self) -> String;
}

#[derive(Debug, Clone)]
#[enum_dispatch]
pub enum HopPathSearchResult {
    TwoHop(TwoHopPathSearchResult),
}

pub fn init_graph(
    dex_json: &[DexJson],
    follow_mints: &[Pubkey],
    hop_paths: Arc<Vec<RwLock<HopPathTypes>>>,
) -> anyhow::Result<()> {
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
    hop_paths.iter().for_each(|mut hop_path| {
        hop_path
            .write()
            .build_graph(edge_identifiers.as_slice(), follow_mint_index.as_slice())
            .expect("初始化Graph失败");
    });
    info!("初始化Graph结束");
    Ok(())
}

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
    pub(crate) fn new(dex_json: &DexJson) -> Vec<Self> {
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

    pub(crate) fn quote(&self, amount_in: u64) -> Option<u64> {
        let pool_id = self.pool_id().unwrap();
        let quoter = get_quoter_type(self.dex_type).ok()?;
        let quote_result: QuoteResult = quoter.quote(amount_in, self.swap_direction, pool_id)?;
        Some(quote_result.amount_out)
    }

    pub(crate) fn get_instruction_material(&self) -> anyhow::Result<InstructionMaterial> {
        let pool_id = self
            .pool_id()
            .ok_or(anyhow!("无法通过index[{}]找到PoolId", self.pool))?;
        get_instruction_builder(&self.dex_type)?
            .convert_to_instruction_material(pool_id, self.swap_direction)
    }

    #[inline]
    pub(crate) fn pool_id(&self) -> Option<&Pubkey> {
        POOL_INDEX
            .get()?
            .get(self.pool)
            .map_or(None, |pool| Some(pool))
    }
}

pub(crate) fn find_pool_position(pool_id: &Pubkey) -> Option<usize> {
    POOL_INDEX.get()?.iter().position(|v| v == pool_id)
}

pub(crate) fn find_mint_position(mint: &Pubkey) -> Option<usize> {
    MINT_INDEX.get()?.iter().position(|v| v == mint)
}

pub(crate) fn find_mint_by_index(index: usize) -> Option<Pubkey> {
    MINT_INDEX.get()?.get(index).cloned()
}
