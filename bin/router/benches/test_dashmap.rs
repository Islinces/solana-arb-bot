use criterion::{black_box, criterion_group, criterion_main, Criterion};
use flurry::HashMap;
use std::sync::Arc;
use std::thread;
use std::time::Duration;
use dashmap::DashMap;

// 测试键值类型
type Key = u64;
type Value = u64;

// 并发写入测试
fn write_benchmark(c: &mut Criterion) {
    let map = Arc::new(DashMap::<Key, Value>::new());

    c.bench_function("flurry_write_4threads", |b| {
        b.iter_custom(|iters| {
            let start = std::time::Instant::now();

            // 启动4个写入线程
            let handles: Vec<_> = (0..15).map(|tid| {
                let map = Arc::clone(&map);
                thread::spawn(move || {
                    for i in 0..iters / 16 {
                        map.insert(0, i * 2);
                    }
                })
            }).collect();

            // 等待所有线程完成
            for h in handles {
                h.join().unwrap();
            }

            start.elapsed()
        })
    });
}

// 混合读写测试 (25% 写 + 75% 读)
fn mixed_benchmark(c: &mut Criterion) {
    let map = Arc::new(DashMap::<Key, Value>::new());
    let write_ratio = 0.5;

    // 预填充数据
    for i in 0..10_000 {
        map.insert(i, i);
    }

    c.bench_function("flurry_mixed_4threads", |b| {
        b.iter_custom(|iters| {
            let start = std::time::Instant::now();

            let handles: Vec<_> = (0..15).map(|tid| {
                let map = Arc::clone(&map);
                thread::spawn(move || {
                    for i in 0..iters / 16 {
                        if (i as f64) < (iters as f64 * write_ratio / 16.0) {
                            // 写入操作
                            map.insert(tid * 1_000_000 + i, i * 2);
                        } else {
                            // 读取操作
                            let key = (i % 10_000) as Key;
                            black_box(map.get(&key));
                        }
                    }
                })
            }).collect();

            for h in handles {
                h.join().unwrap();
            }

            start.elapsed()
        })
    });
}

// 配置基准测试组
criterion_group!{
    name = benches;
    config = Criterion::default()
        .warm_up_time(Duration::from_secs(3))  // 预热3秒
        .measurement_time(Duration::from_secs(10)) // 正式测量10秒
        .sample_size(1000);                        // 采样20次
    targets = write_benchmark, mixed_benchmark
}

criterion_main!(benches);