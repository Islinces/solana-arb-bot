use std::env;
use arb::arb_bot::init_start_data;
use criterion::{criterion_group, criterion_main, Criterion};
use solana_rpc_client::nonblocking::rpc_client::RpcClient;
use solana_sdk::pubkey::Pubkey;
use std::str::FromStr;
use std::sync::Arc;
use std::time::Duration;
use tokio::runtime::Runtime;
use arb::account_cache::get_account_data;
use arb::dex::meteora_dlmm;
use arb::dex::meteora_dlmm::interface::accounts::LbPair;

fn bench_create_instruction(c: &mut Criterion) {
    let mut group = c.benchmark_group("bench_dlmm_quote");
    let mint = Pubkey::from_str("So11111111111111111111111111111111111111112").unwrap();
    let client = Arc::new(RpcClient::new(
        "https://solana-rpc.publicnode.com".to_string(),
    ));
    let rt = Runtime::new().unwrap();
    let _data = rt
        .block_on(init_start_data(
            "/Users/islince/Desktop/workapp/rust/solana-nebula-arb/bin/arb/keypair.bin".to_string(),
            "/Users/islince/Desktop/workapp/rust/solana-nebula-arb/bin/arb/dex_data.json".to_string(),
            &mint,
            &vec![mint.clone()],
            client,
        ))
        .unwrap();
    let pool_id=Pubkey::from_str("627kqiAtYNE4FFKtUcnR9nmDqqCZnQue2ALYRQtnziLR").unwrap();

    group.sample_size(10000);
    group.warm_up_time(Duration::from_secs(5));
    group.measurement_time(std::time::Duration::from_secs(5));
    group.bench_function("dlmm_quote", |b| {
        b.iter(|| {
            let lb_pair = get_account_data::<LbPair>(&pool_id).unwrap();
            meteora_dlmm::quote::quote(10_u64.pow(9),true,&pool_id,lb_pair);
        })
    });
    group.finish();
}

criterion_group!(benches, bench_create_instruction,);
criterion_main!(benches);