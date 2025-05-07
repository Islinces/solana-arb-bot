use criterion::{criterion_group, criterion_main, Criterion};
use dashmap::DashMap;
use router::cache::PoolCache;
use router::dex::meteora_dlmm::meteora_dlmm::MeteoraDLMMSnapshotFetcher;
use router::dex::pump_fun::pump_fun::PumpFunAccountSnapshotFetcher;
use router::dex::raydium_amm::raydium_amm::RaydiumAmmSnapshotFetcher;
use router::dex::raydium_clmm::raydium_clmm::RaydiumCLMMSnapshotFetcher;
use router::dex::{DexData, PoolCacheHolder};
use router::file_db::DexJson;
use router::interface::{AccountSnapshotFetcher, DexType};
use solana_client::nonblocking::rpc_client::RpcClient;
use solana_program::clock::Clock;
use solana_program::pubkey::Pubkey;
use solana_program::sysvar::SysvarId;
use std::collections::HashMap;
use std::str::FromStr;
use std::sync::{Arc, OnceLock};
use std::time::Duration;
use tokio::runtime::Runtime;

static DEX_DATA: OnceLock<DexData> = OnceLock::new();

async fn async_data_preparation() -> PoolCacheHolder {
    // 模拟异步数据准备（例如数据库查询、网络请求）
    create_pool_cache().await
}

fn init_async_data() {
    let rt = Runtime::new().unwrap();
    let data = rt.block_on(async_data_preparation());
    DEX_DATA
        .set(DexData {
            pool_cache_holder: Arc::new(data),
            sol_ata_amount: Arc::new(10000000000000),
        })
        .unwrap()
}

fn bench_route(c: &mut Criterion) {
    let in_mint = Pubkey::from_str("So11111111111111111111111111111111111111112").unwrap();
    let out_mint = Pubkey::from_str("EPjFWdd5AufqSSqeM2qN1xzybapC8G4wEGGkZwyTDt1v").unwrap();
    init_async_data();
    let dex_data = DEX_DATA.get().unwrap();
    let pool_cache = dex_data.pool_cache_holder.clone();
    let pool = Pubkey::from_str("58oQChx4yWmvKdwLLZzBi4ChoCc2fqCUWBkwMihLYQo2").unwrap();
    let (a, b) = pool_cache.build_graph(&in_mint, &pool).unwrap();
    let path0 = Arc::new(a);
    let path1 = Arc::new(b);
    println!("path0: {:#?}, path1: {:#?}", path0, path1);

    let mut group = c.benchmark_group("bench_route");
    group.sample_size(100);
    group.warm_up_time(Duration::from_secs(5));
    group.measurement_time(std::time::Duration::from_secs(5));
    group.bench_function("bench_route", |b| {
        b.iter(||
            async {
                dex_data.find_best_route(in_mint, path0.clone(), path1.clone(), 0)
        });
    });
    let arc = pool_cache.pool_cache.clone().pool_map;
    let amount_in = 10_u64.pow(6);
    let clock = Arc::new(pool_cache.pool_cache.clone().clock);
    group.bench_function("amm_quote", |b| {
        let pool = arc
            .get(&Pubkey::from_str("58oQChx4yWmvKdwLLZzBi4ChoCc2fqCUWBkwMihLYQo2").unwrap())
            .unwrap();
        b.iter(|| {
            DexType::RaydiumAMM.get_quoter().quote(
                amount_in,
                in_mint,
                out_mint,
                pool.value(),
                clock.clone(),
            );
        });
    });
    group.bench_function("clmm_quote", |b| {
        let pool = arc
            .get(&Pubkey::from_str("3ucNos4NbumPLZNWztqGHNFFgkHeRMBQAVemeeomsUxv").unwrap())
            .unwrap();
        b.iter(|| {
            DexType::RaydiumCLmm.get_quoter().quote(
                amount_in,
                in_mint,
                out_mint,
                pool.value(),
                clock.clone(),
            );
        });
    });
    group.bench_function("pump_quote", |b| {
        let pool = arc
            .get(&Pubkey::from_str("Gf7sXMoP8iRw4iiXmJ1nq4vxcRycbGXy5RL8a8LnTd3v").unwrap())
            .unwrap();
        b.iter(|| {
            DexType::PumpFunAMM.get_quoter().quote(
                amount_in,
                in_mint,
                out_mint,
                pool.value(),
                clock.clone(),
            );
        });
    });
    group.bench_function("dlmm_quote", |b| {
        let pool = arc
            .get(&Pubkey::from_str("5rCf1DM8LjKTw4YqhnoLcngyZYeNnQqztScTogYHAS6").unwrap())
            .unwrap();
        b.iter(|| {
            DexType::PumpFunAMM.get_quoter().quote(
                amount_in,
                in_mint,
                out_mint,
                pool.value(),
                clock.clone(),
            );
        });
    });
    group.finish();
}

criterion_group!(benches,bench_route);
criterion_main!(benches);

async fn create_pool_cache() -> PoolCacheHolder {
    let rpc_client = Arc::new(RpcClient::new(
        "https://solana-rpc.publicnode.com".to_string(),
    ));
    let rayidum_amm = RaydiumAmmSnapshotFetcher
        .fetch_snapshot(
            vec![DexJson {
                pool: Pubkey::from_str("58oQChx4yWmvKdwLLZzBi4ChoCc2fqCUWBkwMihLYQo2").unwrap(),
                owner: DexType::RaydiumAMM.get_owner(),
                address_lookup_table_address: Some(
                    Pubkey::from_str("BDqppwFYeMpUicN9xbfoM7FgRnHVW1uTUtrGA7uG2vQg").unwrap(),
                ),
            }],
            rpc_client.clone(),
        )
        .await
        .unwrap();
    let rayidum_clmm = RaydiumCLMMSnapshotFetcher
        .fetch_snapshot(
            vec![DexJson {
                pool: Pubkey::from_str("3ucNos4NbumPLZNWztqGHNFFgkHeRMBQAVemeeomsUxv").unwrap(),
                owner: DexType::RaydiumCLmm.get_owner(),
                address_lookup_table_address: Some(
                    Pubkey::from_str("BDqppwFYeMpUicN9xbfoM7FgRnHVW1uTUtrGA7uG2vQg").unwrap(),
                ),
            }],
            rpc_client.clone(),
        )
        .await
        .unwrap();
    let pump_fun = PumpFunAccountSnapshotFetcher
        .fetch_snapshot(
            vec![DexJson {
                pool: Pubkey::from_str("Gf7sXMoP8iRw4iiXmJ1nq4vxcRycbGXy5RL8a8LnTd3v").unwrap(),
                owner: DexType::PumpFunAMM.get_owner(),
                address_lookup_table_address: Some(
                    Pubkey::from_str("7KgsCbuJAXxELXhpzc9PwX7GenoF3UsuuW71qW1Gr3u9").unwrap(),
                ),
            }],
            rpc_client.clone(),
        )
        .await
        .unwrap();
    let meteora_dlmm = MeteoraDLMMSnapshotFetcher
        .fetch_snapshot(
            vec![DexJson {
                pool: Pubkey::from_str("5rCf1DM8LjKTw4YqhnoLcngyZYeNnQqztScTogYHAS6").unwrap(),
                owner: DexType::RaydiumCLmm.get_owner(),
                address_lookup_table_address: Some(
                    Pubkey::from_str("7KgsCbuJAXxELXhpzc9PwX7GenoF3UsuuW71qW1Gr3u9").unwrap(),
                ),
            }],
            rpc_client.clone(),
        )
        .await
        .unwrap();
    let mut pools = Vec::new();
    pools.extend(rayidum_amm);
    pools.extend(rayidum_clmm);
    pools.extend(pump_fun);
    pools.extend(meteora_dlmm);
    let pool_map = DashMap::new();
    let mut edges = HashMap::new();
    for pool in pools.into_iter() {
        let pool_id = pool.pool_id;
        let mint_pair = pool.token_pair();
        pool_map.insert(pool_id, pool);
        edges
            .entry(mint_pair.0)
            .or_insert(vec![])
            .push((mint_pair.1, pool_id));
        edges
            .entry(mint_pair.1)
            .or_insert(vec![])
            .push((mint_pair.0, pool_id));
    }
    let clock: Clock = bincode::deserialize(
        rpc_client
            .get_account(&Clock::id())
            .await
            .unwrap()
            .data
            .as_ref(),
    )
    .unwrap();
    PoolCacheHolder {
        pool_cache: PoolCache::new(edges, pool_map, clock),
    }
}
