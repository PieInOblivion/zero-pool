#![feature(test)]
extern crate test;

use std::hint::black_box;
use std::sync::OnceLock;
use test::Bencher;

use rayon::prelude::*;
use zero_pool::{
    self, zp_define_task_fn, zp_submit_batch_uniform, zp_task_params, zp_write_indexed,
};

// global pool instance
static ZERO_POOL: OnceLock<zero_pool::ThreadPool> = OnceLock::new();
static RAYON_POOL: OnceLock<rayon::ThreadPool> = OnceLock::new();

fn get_pool() -> &'static zero_pool::ThreadPool {
    ZERO_POOL.get_or_init(|| zero_pool::new())
}

fn get_rayon_pool() -> &'static rayon::ThreadPool {
    RAYON_POOL.get_or_init(|| rayon::ThreadPoolBuilder::new().build().unwrap())
}

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

#[bench]
fn bench_indexed_computation_zeropool(b: &mut Bencher) {
    let pool = get_pool();
    const TASK_COUNT: usize = 1000;
    const WORK_PER_TASK: usize = 100;

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
    let pool = get_rayon_pool();
    const TASK_COUNT: usize = 1000;
    const WORK_PER_TASK: usize = 100;

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
