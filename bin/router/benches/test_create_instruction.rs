use criterion::{criterion_group, criterion_main, Criterion};
use dashmap::DashMap;
use router::arbitrage::jito_arb_executor::{JitoArbExecutor, JitoConfig};
use router::dex::pump_fun::pool_state::PumpFunInstructionItem;
use router::dex::DexQuoteResult;
use router::interface::InstructionItem;
use solana_program::address_lookup_table::AddressLookupTableAccount;
use solana_program::hash::Hash;
use solana_program::pubkey::Pubkey;
use solana_sdk::signature::Keypair;
use std::collections::HashMap;
use std::sync::Arc;
use std::time::Duration;
use moka::sync::Cache;
use tokio::sync::Mutex;

fn bench_create_instruction(c: &mut Criterion) {
    let mut group = c.benchmark_group("bench_create_instruction");
    let items = create_item();
    let hash = Hash::default();
    let map = DashMap::new();
    map.insert(spl_token::native_mint::id(),Pubkey::default());
    let executor = JitoArbExecutor::new(
        None,
        Keypair::new(),
        Arc::new(map),
        Arc::new(Mutex::new(hash)),
        JitoConfig {
            jito_region: "".to_string(),
            jito_uuid: None,
        },
    );
    group.sample_size(10000);
    group.warm_up_time(Duration::from_secs(5));
    group.measurement_time(std::time::Duration::from_secs(5));
    group.bench_function("bench_route", |b| {
        b.iter(|| {
            executor.build_jupiter_swap_ix(items.clone(),100);
        })
    });
    group.finish();
}

criterion_group!(benches, bench_create_instruction,);
criterion_main!(benches);

fn create_item() -> DexQuoteResult {
    DexQuoteResult {
        instruction_items: vec![
            InstructionItem::PumpFunAMM(PumpFunInstructionItem {
                pool_id: Pubkey::default(),
                mint_0: Pubkey::default(),
                mint_1: Pubkey::default(),
                mint_0_vault: Pubkey::default(),
                mint_1_vault: Pubkey::default(),
                alt: AddressLookupTableAccount {
                    key: Pubkey::default(),
                    addresses: vec![],
                },
                zero_to_one: true,
            }),
            InstructionItem::PumpFunAMM(PumpFunInstructionItem {
                pool_id: Pubkey::default(),
                mint_0: Pubkey::default(),
                mint_1: Pubkey::default(),
                mint_0_vault: Pubkey::default(),
                mint_1_vault: Pubkey::default(),
                alt: AddressLookupTableAccount {
                    key: Pubkey::default(),
                    addresses: vec![],
                },
                zero_to_one: false,
            }),
        ],
        amount_in_mint: Pubkey::default(),
        amount_in: 100000,
        amount_out: 1000000,
        profit: 100,
        ..Default::default()
    }
}
