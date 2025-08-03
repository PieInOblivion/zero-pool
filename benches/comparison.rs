#![feature(test)]
extern crate test;

use std::hint::black_box;
use test::Bencher;

use rayon::prelude::*;
use zero_pool::{
    self, zp_define_task_fn, zp_submit_batch_uniform, zp_task_params, zp_write_indexed,
};

const TASK_COUNT: usize = 1000;
const WORK_PER_TASK: usize = 100;

// task parameters: work amount, index to write to, and results vector
zp_task_params! {
    ComputeTask {
        work_size: usize,
        index: usize,
        results: *mut Vec<u64>,
    }
}

// task function that does some computation and writes result to an index
zp_define_task_fn!(compute_task_fn, ComputeTask, |params| {
    let mut sum = 0u64;

    for i in 0..params.work_size {
        sum = sum.wrapping_add((i as u64).wrapping_mul(17).wrapping_add(23));
    }

    // write result directly to the pre-allocated vector at the specified index
    zp_write_indexed!(params.results, params.index, sum);
});

zp_task_params! {
    EmptyTask {
        index: usize,
        results: *mut Vec<u64>,
    }
}

zp_define_task_fn!(empty_task_fn, EmptyTask, |params| {
    // just write a constant to make sure it's not optimised out
    zp_write_indexed!(params.results, params.index, 42u64);
});

#[bench]
fn bench_indexed_computation_zeropool(b: &mut Bencher) {
    let pool = zero_pool::new();
    
    b.iter(|| {
        // allocate results vector
        let mut results = vec![0u64; TASK_COUNT];

        // create all task structs, each with its target index
        let mut tasks = Vec::with_capacity(TASK_COUNT);
        for i in 0..TASK_COUNT {
            tasks.push(ComputeTask::new(WORK_PER_TASK, i, &mut results));
        }

        // submit uniform batch and wait for completion
        let batch = zp_submit_batch_uniform!(pool, tasks, compute_task_fn);
        batch.wait();

        black_box(results);
    });
}

#[bench]
fn bench_indexed_computation_rayon(b: &mut Bencher) {
    let pool = rayon::ThreadPoolBuilder::new().build().unwrap();

    b.iter(|| {
        let results: Vec<u64> = pool.install(|| {
            (0..TASK_COUNT)
                .into_par_iter()
                .map(|_| {
                    let mut sum = 0u64;

                    // same computational work as zero-pool version
                    for i in 0..WORK_PER_TASK {
                        sum = sum.wrapping_add((i as u64).wrapping_mul(17).wrapping_add(23));
                    }

                    sum
                })
                .collect()
        });

        black_box(results);
    });
}

#[bench]
fn bench_task_overhead_zeropool(b: &mut Bencher) {
    let pool = zero_pool::new();

    b.iter(|| {
        let mut results = vec![0u64; TASK_COUNT];

        let mut tasks = Vec::with_capacity(TASK_COUNT);
        for i in 0..TASK_COUNT {
            tasks.push(EmptyTask::new(i, &mut results));
        }

        let batch = zp_submit_batch_uniform!(pool, tasks, empty_task_fn);
        batch.wait();

        black_box(results);
    });
}

#[bench]
fn bench_task_overhead_rayon(b: &mut Bencher) {
    let pool = rayon::ThreadPoolBuilder::new().build().unwrap();

    b.iter(|| {
        let results: Vec<u64> = pool.install(|| {
            (0..TASK_COUNT)
                .into_par_iter()
                .map(|_| 42u64)
                .collect()
        });

        black_box(results);
    });
}

// zero pool specific optimisation, tasks only needs to be created once
// create the whole 
#[bench]
fn bench_indexed_computation_zeropool_optimised(b: &mut Bencher) {
    let pool = zero_pool::new();

    let mut results = vec![0u64; TASK_COUNT];
    let mut tasks = Vec::with_capacity(TASK_COUNT);
    for i in 0..TASK_COUNT {
        tasks.push((ComputeTask::new(WORK_PER_TASK, i, &mut results), compute_task_fn));
    }
    
    b.iter(|| {
        let batch = zp_submit_batch_uniform!(pool, tasks, compute_task_fn);
        batch.wait();
        black_box(results.clone());
    });
}

#[bench]
fn bench_indexed_computation_rayon_optimised(b: &mut Bencher) {
    let pool = rayon::ThreadPoolBuilder::new().build().unwrap();

    let mut results = vec![0u64; TASK_COUNT];

    b.iter(|| {
        pool.install(|| {
            results
                .par_iter_mut()
                .for_each(|slot| {
                    let mut sum = 0u64;
                    for i in 0..WORK_PER_TASK {
                        sum = sum.wrapping_add((i as u64).wrapping_mul(17).wrapping_add(23));
                    }
                    *slot = sum;
                })
        });
        black_box(results.clone());
    });
}

#[bench]
fn bench_task_overhead_zeropool_optimised(b: &mut Bencher) {
    let pool = zero_pool::new();

    let mut results = vec![0u64; TASK_COUNT];
    let mut tasks = Vec::with_capacity(TASK_COUNT);
    for i in 0..TASK_COUNT {
        tasks.push((EmptyTask::new(i, &mut results), empty_task_fn));
    }

    b.iter(|| {
        let batch = zp_submit_batch_uniform!(pool, tasks, empty_task_fn);
        batch.wait();
        black_box(results.clone());
    });
}

#[bench]
fn bench_task_overhead_rayon_optimised(b: &mut Bencher) {
    let pool = rayon::ThreadPoolBuilder::new().build().unwrap();

    let mut results = vec![0u64; TASK_COUNT];

    b.iter(|| {
        results = pool.install(|| {
            (0..TASK_COUNT)
                .into_par_iter()
                .map(|_| 42u64)
                .collect()
        });

        black_box(results.clone());
    });
}