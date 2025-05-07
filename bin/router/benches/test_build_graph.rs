use criterion::{black_box, criterion_group, criterion_main, Criterion};
use dashmap::DashMap;
use moka::sync::Cache;
use router::dex::{DexData, Path};
use router::interface::DexType;
use solana_program::pubkey::Pubkey;
use std::collections::HashMap;
use std::str::FromStr;
use std::sync::{Arc, OnceLock};
use std::thread;
use std::time::Duration;

static DEX_DATA: OnceLock<Cache<Pubkey, (Vec<Path>, Vec<Path>)>> = OnceLock::new();

fn build_graph(c: &mut Criterion) {
    let result: HashMap<String, Vec<(String, String)>> = serde_json::from_str("{\"So11111111111111111111111111111111111111112\":[[\"EPjFWdd5AufqSSqeM2qN1xzybapC8G4wEGGkZwyTDt1v\",\"58oQChx4yWmvKdwLLZzBi4ChoCc2fqCUWBkwMihLYQo2\"],[\"9BB6NFEcjBCtnNLFko2FqVQBq8HHM13kCyYcdQbgpump\",\"Bzc9NZfMqkXR6fz1DBph7BDf9BroyEf6pnzESP7v5iiw\"],[\"EPjFWdd5AufqSSqeM2qN1xzybapC8G4wEGGkZwyTDt1v\",\"Gf7sXMoP8iRw4iiXmJ1nq4vxcRycbGXy5RL8a8LnTd3v\"],[\"9BB6NFEcjBCtnNLFko2FqVQBq8HHM13kCyYcdQbgpump\",\"AmmpSnW5xVeKHTAU9fMjyKEMPgrzmUj3ah5vgvHhAB5J\"],[\"EPjFWdd5AufqSSqeM2qN1xzybapC8G4wEGGkZwyTDt1v\",\"6MUjnGffYaqcHeqv4nNemUQVNMpJab3W2NV9bfPj576c\"],[\"EPjFWdd5AufqSSqeM2qN1xzybapC8G4wEGGkZwyTDt1v\",\"3ucNos4NbumPLZNWztqGHNFFgkHeRMBQAVemeeomsUxv\"],[\"EPjFWdd5AufqSSqeM2qN1xzybapC8G4wEGGkZwyTDt1v\",\"2QdhepnKRTLjjSqPL1PtKNwqrUkoLee5Gqs8bvZhRdMv\"],[\"EPjFWdd5AufqSSqeM2qN1xzybapC8G4wEGGkZwyTDt1v\",\"CYbD9RaToYMtWKA7QZyoLahnHdWq553Vm62Lh6qWtuxq\"],[\"EPjFWdd5AufqSSqeM2qN1xzybapC8G4wEGGkZwyTDt1v\",\"EXHyQxMSttcvLPwjENnXCPZ8GmLjJYHtNBnAkcFeFKMn\"],[\"EPjFWdd5AufqSSqeM2qN1xzybapC8G4wEGGkZwyTDt1v\",\"8sLbNZoA1cfnvMJLPfp98ZLAnFSYCFApfJKMbiXNLwxj\"],[\"9BB6NFEcjBCtnNLFko2FqVQBq8HHM13kCyYcdQbgpump\",\"FdjBRWXzieV1Qtn8FLDWWk2HK1HSQWYduo8y4F1e8GWu\"],[\"9BB6NFEcjBCtnNLFko2FqVQBq8HHM13kCyYcdQbgpump\",\"6U4TBh3aJgiJ5EqCDEua4rP75HsqcfHapMKhhyuTqGuo\"],[\"7GCihgDB8fe6KNjn2MYtkzZcRjQy3t9GHdC8uHYmW2hr\",\"dgfJKacfubDGfX3mhEk1JKaK9qx38SpLzM2wNdgX2fD\"],[\"7GCihgDB8fe6KNjn2MYtkzZcRjQy3t9GHdC8uHYmW2hr\",\"43xBeK8mZtssDjFLdrpr2muH5AYQJRVFb22y5rHvfo9C\"],[\"EPjFWdd5AufqSSqeM2qN1xzybapC8G4wEGGkZwyTDt1v\",\"5BKxfWMbmYBAEWvyPZS9esPducUba9GqyMjtLCfbaqyF\"],[\"EPjFWdd5AufqSSqeM2qN1xzybapC8G4wEGGkZwyTDt1v\",\"6YuRXMMF4W8zo216CP1A7iiE627GdDaitCRS2iXEdM2q\"],[\"EPjFWdd5AufqSSqeM2qN1xzybapC8G4wEGGkZwyTDt1v\",\"GJA5EuRNESccQ86yTzBNKNz2JxMZhW6Z6xbmQyk7mLjQ\"],[\"EPjFWdd5AufqSSqeM2qN1xzybapC8G4wEGGkZwyTDt1v\",\"AdewTcFb9UKiy7sc7ypW2fRC5TDjkuMHfsjtGcipAset\"],[\"EPjFWdd5AufqSSqeM2qN1xzybapC8G4wEGGkZwyTDt1v\",\"HTvjzsfX3yU6BUodCjZ5vZkUrAxMDTrBs3CJaq43ashR\"],[\"EPjFWdd5AufqSSqeM2qN1xzybapC8G4wEGGkZwyTDt1v\",\"9Q1njS4j8svdjCnGd2xJn7RAkqrJ2vqjaPs3sXRZ6UR7\"],[\"EPjFWdd5AufqSSqeM2qN1xzybapC8G4wEGGkZwyTDt1v\",\"DJJzSUx5gEqDJhTyE8tvZtkkHG7R7v6EoyZXN32aAsXF\"],[\"EPjFWdd5AufqSSqeM2qN1xzybapC8G4wEGGkZwyTDt1v\",\"FK3xj3tMeX7ziVNG7mEDi6Ej6UrQAJ68MaBvbL7qVkUA\"]],\"9BB6NFEcjBCtnNLFko2FqVQBq8HHM13kCyYcdQbgpump\":[[\"So11111111111111111111111111111111111111112\",\"Bzc9NZfMqkXR6fz1DBph7BDf9BroyEf6pnzESP7v5iiw\"],[\"So11111111111111111111111111111111111111112\",\"AmmpSnW5xVeKHTAU9fMjyKEMPgrzmUj3ah5vgvHhAB5J\"],[\"So11111111111111111111111111111111111111112\",\"FdjBRWXzieV1Qtn8FLDWWk2HK1HSQWYduo8y4F1e8GWu\"],[\"So11111111111111111111111111111111111111112\",\"6U4TBh3aJgiJ5EqCDEua4rP75HsqcfHapMKhhyuTqGuo\"]],\"7GCihgDB8fe6KNjn2MYtkzZcRjQy3t9GHdC8uHYmW2hr\":[[\"So11111111111111111111111111111111111111112\",\"dgfJKacfubDGfX3mhEk1JKaK9qx38SpLzM2wNdgX2fD\"],[\"So11111111111111111111111111111111111111112\",\"43xBeK8mZtssDjFLdrpr2muH5AYQJRVFb22y5rHvfo9C\"]],\"EPjFWdd5AufqSSqeM2qN1xzybapC8G4wEGGkZwyTDt1v\":[[\"So11111111111111111111111111111111111111112\",\"58oQChx4yWmvKdwLLZzBi4ChoCc2fqCUWBkwMihLYQo2\"],[\"So11111111111111111111111111111111111111112\",\"Gf7sXMoP8iRw4iiXmJ1nq4vxcRycbGXy5RL8a8LnTd3v\"],[\"So11111111111111111111111111111111111111112\",\"6MUjnGffYaqcHeqv4nNemUQVNMpJab3W2NV9bfPj576c\"],[\"So11111111111111111111111111111111111111112\",\"3ucNos4NbumPLZNWztqGHNFFgkHeRMBQAVemeeomsUxv\"],[\"So11111111111111111111111111111111111111112\",\"2QdhepnKRTLjjSqPL1PtKNwqrUkoLee5Gqs8bvZhRdMv\"],[\"So11111111111111111111111111111111111111112\",\"CYbD9RaToYMtWKA7QZyoLahnHdWq553Vm62Lh6qWtuxq\"],[\"So11111111111111111111111111111111111111112\",\"EXHyQxMSttcvLPwjENnXCPZ8GmLjJYHtNBnAkcFeFKMn\"],[\"So11111111111111111111111111111111111111112\",\"8sLbNZoA1cfnvMJLPfp98ZLAnFSYCFApfJKMbiXNLwxj\"],[\"So11111111111111111111111111111111111111112\",\"5BKxfWMbmYBAEWvyPZS9esPducUba9GqyMjtLCfbaqyF\"],[\"So11111111111111111111111111111111111111112\",\"6YuRXMMF4W8zo216CP1A7iiE627GdDaitCRS2iXEdM2q\"],[\"So11111111111111111111111111111111111111112\",\"GJA5EuRNESccQ86yTzBNKNz2JxMZhW6Z6xbmQyk7mLjQ\"],[\"So11111111111111111111111111111111111111112\",\"AdewTcFb9UKiy7sc7ypW2fRC5TDjkuMHfsjtGcipAset\"],[\"So11111111111111111111111111111111111111112\",\"HTvjzsfX3yU6BUodCjZ5vZkUrAxMDTrBs3CJaq43ashR\"],[\"So11111111111111111111111111111111111111112\",\"9Q1njS4j8svdjCnGd2xJn7RAkqrJ2vqjaPs3sXRZ6UR7\"],[\"So11111111111111111111111111111111111111112\",\"DJJzSUx5gEqDJhTyE8tvZtkkHG7R7v6EoyZXN32aAsXF\"],[\"So11111111111111111111111111111111111111112\",\"FK3xj3tMeX7ziVNG7mEDi6Ej6UrQAJ68MaBvbL7qVkUA\"]]}").unwrap();
    let mut edges = HashMap::with_capacity(100);
    result.into_iter().for_each(|(k, v)| {
        edges.insert(
            Pubkey::from_str(&k).unwrap(),
            v.iter()
                .map(|p| {
                    (
                        Pubkey::from_str(&p.0).unwrap(),
                        Pubkey::from_str(&p.1).unwrap(),
                    )
                })
                .collect::<Vec<_>>(),
        );
    });
    let amount_in_mint = &Pubkey::from_str("So11111111111111111111111111111111111111112").unwrap();
    let update_pool = &Pubkey::from_str("58oQChx4yWmvKdwLLZzBi4ChoCc2fqCUWBkwMihLYQo2").unwrap();
    let cache: Cache<Pubkey, (Vec<Path>, Vec<Path>)> = Cache::builder().max_capacity(100).build();
    let mut group = c.benchmark_group("bench_route");
    group.sample_size(10000);
    group.warm_up_time(Duration::from_secs(10));
    group.measurement_time(std::time::Duration::from_secs(10));
    group.bench_function("clmm_quote", |b| {
        b.iter(|| build(amount_in_mint, update_pool, &edges, &cache));
    });
}

fn build(
    amount_in_mint: &Pubkey,
    update_pool: &Pubkey,
    edges: &HashMap<Pubkey, Vec<(Pubkey, Pubkey)>>,
    cache: &Cache<Pubkey, (Vec<Path>, Vec<Path>)>,
) -> (Vec<Path>, Vec<Path>) {
    cache
        .entry(amount_in_mint.clone())
        .or_insert_with(|| {
            let mut positive_paths = Vec::with_capacity(edges.len() / 2);
            let mut reverse_paths = Vec::with_capacity(edges.len() / 2);
            // 返回的path已经包含了update_pool作为输入和作为输出的path
            if let Some(first_pools) = edges.get(&amount_in_mint) {
                for (f_pool_out_mint, f_pool) in first_pools {
                    if let Some(second_pools) = edges.get(f_pool_out_mint) {
                        for (s_pool_out_mint, s_pool) in second_pools {
                            if amount_in_mint != s_pool_out_mint || f_pool == s_pool {
                                continue;
                            }
                            // 要包含更新的池子
                            if f_pool != update_pool && s_pool != update_pool {
                                continue;
                            }
                            if f_pool == update_pool {
                                positive_paths.push(Path {
                                    path: vec![*f_pool, *s_pool],
                                })
                            } else {
                                reverse_paths.push(Path {
                                    path: vec![*f_pool, *s_pool],
                                })
                            }
                        }
                    }
                }
            }
            (positive_paths, reverse_paths)
        })
        .value()
        .clone()
}

// 配置基准测试组
criterion_group! {benches,build_graph}
criterion_main!(benches);
