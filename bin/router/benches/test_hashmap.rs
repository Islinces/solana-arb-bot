use criterion::{black_box, criterion_group, criterion_main, Criterion};
use dashmap::DashMap;
use flurry::HashMap;
use std::sync::Arc;
use std::thread;
use std::time::Duration;

// 测试键值类型
type Key = u64;
type Value = u64;

const THREADS: u64 = 16;
const THREADS_INDEX: u64 = 15;
const THREADS_F: f64 = 16.0;
const DATA_SIZE: u64 = 10_000;
const RATIO: f64 = 0.5;

// 并发写入测试
fn flurry_write_benchmark(c: &mut Criterion) {
    let map = Arc::new(HashMap::<Key, Value>::new());
    let i1 = THREADS - 1;
    c.bench_function("flurry_write", |b| {
        b.iter_custom(|iters| {
            let start = std::time::Instant::now();

            // 启动4个写入线程
            let handles: Vec<_> = (0..THREADS_INDEX)
                .map(|tid| {
                    let map = Arc::clone(&map);
                    thread::spawn(move || {
                        let guard = map.guard();
                        for i in 0..iters / THREADS {
                            map.insert(0, i * 2, &guard);
                        }
                    })
                })
                .collect();

            // 等待所有线程完成
            for h in handles {
                h.join().unwrap();
            }

            start.elapsed()
        })
    });
}

// 混合读写测试 (25% 写 + 75% 读)
fn flurry_mixed_benchmark(c: &mut Criterion) {
    let map = Arc::new(HashMap::<Key, Value>::new());

    // 预填充数据
    let guard = map.guard();
    for i in 0..DATA_SIZE {
        map.insert(i, i, &guard);
    }
    drop(guard);

    c.bench_function("flurry_mixed", |b| {
        b.iter_custom(|iters| {
            let start = std::time::Instant::now();

            let handles: Vec<_> = (0..THREADS_INDEX)
                .map(|tid| {
                    let map = Arc::clone(&map);
                    thread::spawn(move || {
                        let guard = map.guard();
                        for i in 0..iters / THREADS_INDEX {
                            if (i as f64) < (iters as f64 * RATIO / THREADS_F) {
                                // 写入操作
                                map.insert(tid * 1_000_000 + i, i * 2, &guard);
                            } else {
                                // 读取操作
                                let key = (i % DATA_SIZE) as Key;
                                black_box(map.get(&key, &guard));
                            }
                        }
                    })
                })
                .collect();

            for h in handles {
                h.join().unwrap();
            }

            start.elapsed()
        })
    });
}

fn dashmap_write_benchmark(c: &mut Criterion) {
    let map = Arc::new(DashMap::<Key, Value>::new());

    c.bench_function("dashmap_write", |b| {
        b.iter_custom(|iters| {
            let start = std::time::Instant::now();

            // 启动4个写入线程
            let handles: Vec<_> = (0..THREADS_INDEX)
                .map(|tid| {
                    let map = Arc::clone(&map);
                    thread::spawn(move || {
                        for i in 0..iters / THREADS {
                            map.insert(0, i * 2);
                        }
                    })
                })
                .collect();

            // 等待所有线程完成
            for h in handles {
                h.join().unwrap();
            }

            start.elapsed()
        })
    });
}

// 混合读写测试 (25% 写 + 75% 读)
fn dashmap_mixed_benchmark(c: &mut Criterion) {
    let map = Arc::new(DashMap::<Key, Value>::new());

    // 预填充数据
    for i in 0..DATA_SIZE {
        map.insert(i, i);
    }

    c.bench_function("dashmap_mixed", |b| {
        b.iter_custom(|iters| {
            let start = std::time::Instant::now();

            let handles: Vec<_> = (0.. THREADS_INDEX)
                .map(|tid| {
                    let map = Arc::clone(&map);
                    thread::spawn(move || {
                        for i in 0..iters / THREADS {
                            if (i as f64) < (iters as f64 * RATIO / THREADS_F) {
                                // 写入操作
                                map.insert(tid * 1_000_000 + i, i * 2);
                            } else {
                                // 读取操作
                                let key = (i % 10_000) as Key;
                                black_box(map.get(&key));
                            }
                        }
                    })
                })
                .collect();

            for h in handles {
                h.join().unwrap();
            }

            start.elapsed()
        })
    });
}

// 配置基准测试组
criterion_group! {
    name = benches;
    config = Criterion::default()
        .warm_up_time(Duration::from_secs(3))  // 预热3秒
        .measurement_time(Duration::from_secs(10)) // 正式测量10秒
        .sample_size(1000);                        // 采样20次
    targets = flurry_write_benchmark, flurry_mixed_benchmark,dashmap_write_benchmark,dashmap_mixed_benchmark
}

criterion_main!(benches);
