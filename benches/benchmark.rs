#[macro_use]
extern crate criterion;
extern crate rayon;
extern crate shortid;
extern crate uuid;

use criterion::black_box;
use criterion::Criterion;

use rayon::prelude::*;
use uuid::v1::{Context, Timestamp};

static CONTEXT: Context = Context::new(0);

fn short_128(id: &[u8; 4]) -> u128 {
    u128::from_be_bytes(shortid::next_short_128(id).unwrap())
}

fn short_128_benchmark(c: &mut Criterion) {
    c.bench_function("short 128", |b| {
        b.iter(|| short_128(black_box(&[1, 2, 3, 4])))
    });
}

fn uuidv1(node_id: &[u8; 6]) -> u128 {
    use std::time::{SystemTime, UNIX_EPOCH};
    let time = SystemTime::now().duration_since(UNIX_EPOCH).unwrap();
    let ts = Timestamp::from_unix(&CONTEXT, time.as_secs(), time.subsec_nanos());
    uuid::Uuid::new_v1(ts, node_id).unwrap().as_u128()
}

fn uuidv1_benchmark(c: &mut Criterion) {
    c.bench_function("uuidv1", |b| {
        b.iter(|| uuidv1(black_box(&[1, 2, 3, 4, 5, 6])))
    });
}

fn myuuidv1(node_id: &[u8; 6]) -> u128 {
    u128::from_be_bytes(shortid::uuidv1(node_id).unwrap())
}

fn myuuidv1_benchmark(c: &mut Criterion) {
    c.bench_function("uuidv1", |b| {
        b.iter(|| myuuidv1(black_box(&[1, 2, 3, 4, 5, 6])))
    });
}

fn short_128_benchmark_parallel(c: &mut Criterion) {
    c.bench_function("short 128 parallel", |b| {
        b.iter(|| {
            let result: Vec<_> = (0u32..1000)
                .into_par_iter()
                .map(|v: u32| short_128(black_box(&v.to_le_bytes())))
                .collect();
            result
        })
    });
}

fn uuidv1_benchmark_parallel(c: &mut Criterion) {
    c.bench_function("uuidv1 parallel", |b| {
        b.iter(|| {
            let result: Vec<_> = (0u32..1000)
                .into_par_iter()
                .map(|v: u32| {
                    let b = v.to_le_bytes();
                    uuidv1(black_box(&[b[0], b[1], b[2], b[3], 0, 0]))
                })
                .collect();
            result
        })
    });
}

fn myuuidv1_benchmark_parallel(c: &mut Criterion) {
    c.bench_function("uuidv1 parallel", |b| {
        b.iter(|| {
            let result: Vec<_> = (0u32..1000)
                .into_par_iter()
                .map(|v: u32| {
                    let b = v.to_le_bytes();
                    myuuidv1(black_box(&[b[0], b[1], b[2], b[3], 0, 0]))
                })
                .collect();
            result
        })
    });
}

criterion_group!(
    benches,
    short_128_benchmark,
    uuidv1_benchmark,
    myuuidv1_benchmark,
    short_128_benchmark_parallel,
    uuidv1_benchmark_parallel,
    myuuidv1_benchmark_parallel
);
criterion_main!(benches);
