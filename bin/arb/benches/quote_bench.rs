use arb::global_cache::get_account_data;
use arb::arb_bot::init_start_data;
use arb::dex::meteora_dlmm::interface::accounts::LbPair;
use arb::dex::pump_fun::state::Pool;
use arb::dex::raydium_amm::state::AmmInfo;
use arb::dex::raydium_clmm::state::PoolState;
use arb::dex::{meteora_dlmm, pump_fun, raydium_amm, raydium_clmm};
use criterion::{criterion_group, criterion_main, Criterion};
use solana_rpc_client::nonblocking::rpc_client::RpcClient;
use solana_sdk::pubkey::Pubkey;
use std::env;
use std::str::FromStr;
use std::sync::Arc;
use std::time::Duration;
use tokio::runtime::Runtime;

fn bench_create_instruction(c: &mut Criterion) {
    println!("{:?}", env::current_dir().unwrap());
    let mut group = c.benchmark_group("bench_dlmm_quote");
    let mint = Pubkey::from_str("So11111111111111111111111111111111111111112").unwrap();
    let client = Arc::new(RpcClient::new(
        "https://solana-rpc.publicnode.com".to_string(),
    ));
    let rt = Runtime::new().unwrap();
    let _data = rt
        .block_on(init_start_data(
            "keypair.bin".to_string(),
            "dex_data.json".to_string(),
            &mint,
            &vec![mint.clone()],
            client,
        ))
        .unwrap();
    let dlmm_pool_id = Pubkey::from_str("627kqiAtYNE4FFKtUcnR9nmDqqCZnQue2ALYRQtnziLR").unwrap();
    let amm_pool_id = Pubkey::from_str("58oQChx4yWmvKdwLLZzBi4ChoCc2fqCUWBkwMihLYQo2").unwrap();
    let clmm_pool_id = Pubkey::from_str("6MUjnGffYaqcHeqv4nNemUQVNMpJab3W2NV9bfPj576c").unwrap();
    let pumpfun_pool_id = Pubkey::from_str("Gf7sXMoP8iRw4iiXmJ1nq4vxcRycbGXy5RL8a8LnTd3v").unwrap();

    group.sample_size(10000);
    group.warm_up_time(Duration::from_secs(5));
    group.measurement_time(std::time::Duration::from_secs(5));
    group.bench_function("dlmm_quote", |b| {
        b.iter(|| {
            let lb_pair = get_account_data::<LbPair>(&dlmm_pool_id).unwrap();
            meteora_dlmm::quote::quote(10_u64.pow(9), true, &dlmm_pool_id, lb_pair);
        })
    });
    group.bench_function("amm_quote", |b| {
        b.iter(|| {
            let pool = get_account_data::<AmmInfo>(&amm_pool_id).unwrap();
            raydium_amm::quote::quote(10_u64.pow(9), true, pool);
        })
    });
    group.bench_function("clmm_quote", |b| {
        b.iter(|| {
            let pool = get_account_data::<PoolState>(&clmm_pool_id).unwrap();
            raydium_clmm::quote::quote(10_u64.pow(9), true, &clmm_pool_id, pool);
        })
    });
    group.bench_function("pumpfun_quote", |b| {
        b.iter(|| {
            let pool = get_account_data::<Pool>(&pumpfun_pool_id).unwrap();
            pump_fun::quote::quote(10_u64.pow(9), false, pool);
        })
    });
    group.finish();
}

criterion_group!(benches, bench_create_instruction,);
criterion_main!(benches);
