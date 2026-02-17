#![feature(test)]
extern crate test;

use std::hint::black_box;
use test::Bencher;

use rayon::prelude::*;
use zero_pool::ZeroPool;

const TASK_COUNT: usize = 1000;
const INDIVIDUAL_TASK_COUNT: usize = 1000;

struct HeavyComputeTask {
    seed: u64,
    result: *mut u64,
}

// heavy compute task function with variable work based on seed
fn heavy_compute_task_fn(params: &HeavyComputeTask) {
    // use fixed work amount instead of variable
    let work_amount = 30000;

    let mut sum = 0u64;
    let mut x = params.seed;

    for _ in 0..work_amount {
        // complex computation
        x = x.wrapping_mul(1664525).wrapping_add(1013904223);
        sum = sum.wrapping_add(x);

        // some branching to make it less predictable
        if x.is_multiple_of(3) {
            sum = sum.wrapping_mul(17);
        } else if x.is_multiple_of(7) {
            sum = sum.wrapping_add(x >> 8);
        }
    }

    unsafe {
        *params.result = sum;
    }
}

struct IndexTask {
    result: *mut u64,
}

fn index_task_fn(params: &IndexTask) {
    // just write a constant to make sure it's not optimised out
    unsafe {
        *params.result = 42u64;
    }
}

#[bench]
fn bench_heavy_compute_zeropool(b: &mut Bencher) {
    let pool = ZeroPool::new();

    // generate seeds for consistent random work distribution
    let seeds: Vec<u64> = (0..TASK_COUNT)
        .map(|i| {
            let mut seed = i as u64;
            seed = seed.wrapping_mul(1103515245).wrapping_add(12345);
            seed
        })
        .collect();

    b.iter(|| {
        let mut results = vec![0u64; TASK_COUNT];

        let mut tasks = Vec::with_capacity(TASK_COUNT);
        for (i, &seed) in seeds.iter().enumerate().take(TASK_COUNT) {
            tasks.push(HeavyComputeTask {
                seed,
                result: &mut results[i],
            });
        }

        let batch = pool.submit_batch(heavy_compute_task_fn, &tasks);
        batch.wait();

        black_box(results);
    });
}

#[bench]
fn bench_heavy_compute_rayon(b: &mut Bencher) {
    let pool = rayon::ThreadPoolBuilder::new().build().unwrap();

    // generate seeds for consistent random work distribution
    let seeds: Vec<u64> = (0..TASK_COUNT)
        .map(|i| {
            let mut seed = i as u64;
            seed = seed.wrapping_mul(1103515245).wrapping_add(12345);
            seed
        })
        .collect();

    b.iter(|| {
        let results: Vec<u64> = pool.install(|| {
            seeds
                .par_iter()
                .map(|&seed| {
                    // use fixed work amount instead of variable
                    let work_amount = 30000; // consistent work per task

                    let mut sum = 0u64;
                    let mut x = seed;

                    for _ in 0..work_amount {
                        x = x.wrapping_mul(1664525).wrapping_add(1013904223);
                        sum = sum.wrapping_add(x);

                        if x.is_multiple_of(3) {
                            sum = sum.wrapping_mul(17);
                        } else if x.is_multiple_of(7) {
                            sum = sum.wrapping_add(x >> 8);
                        }
                    }

                    sum
                })
                .collect()
        });

        black_box(results);
    });
}

#[bench]
fn bench_individual_tasks_zeropool(b: &mut Bencher) {
    let pool = ZeroPool::new();

    b.iter(|| {
        let mut results = vec![0u64; INDIVIDUAL_TASK_COUNT];
        let mut tasks = Vec::with_capacity(INDIVIDUAL_TASK_COUNT);
        let mut futures = Vec::with_capacity(INDIVIDUAL_TASK_COUNT);

        for result in results.iter_mut() {
            tasks.push(IndexTask { result });
        }

        // submit individual tasks
        for task in tasks.iter() {
            let future = pool.submit_task(index_task_fn, task);
            futures.push(future);
        }

        // wait for all
        for future in futures {
            future.wait();
        }

        black_box(results);
    });
}

#[bench]
fn bench_individual_tasks_rayon(b: &mut Bencher) {
    let pool = rayon::ThreadPoolBuilder::new().build().unwrap();

    b.iter(|| {
        let mut results = vec![0u64; INDIVIDUAL_TASK_COUNT];

        pool.install(|| {
            rayon::scope(|s| {
                for result in results.iter_mut() {
                    s.spawn(move |_| {
                        *result = 42u64;
                    });
                }
            });
        });

        black_box(results);
    });
}

#[bench]
fn bench_task_overhead_zeropool(b: &mut Bencher) {
    let pool = ZeroPool::new();

    b.iter(|| {
        let mut results = vec![0u64; TASK_COUNT];

        let mut tasks = Vec::with_capacity(TASK_COUNT);
        for result in results.iter_mut().take(TASK_COUNT) {
            tasks.push(IndexTask { result });
        }

        let batch = pool.submit_batch(index_task_fn, &tasks);
        batch.wait();

        black_box(results);
    });
}

#[bench]
fn bench_task_overhead_rayon(b: &mut Bencher) {
    let pool = rayon::ThreadPoolBuilder::new().build().unwrap();

    b.iter(|| {
        let results: Vec<u64> =
            pool.install(|| (0..TASK_COUNT).into_par_iter().map(|_| 42u64).collect());

        black_box(results);
    });
}
