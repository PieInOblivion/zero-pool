use std::hint::black_box;
use criterion::{criterion_group, criterion_main, Criterion};
use rayon::prelude::*;
use zero_pool::{self, zp_task_params, zp_define_task_fn, zp_submit_batch_uniform};

// Task parameters for benchmarks
zp_task_params! {
    SimpleTask {
        iterations: usize,
        result: *mut u64,
    }
}

zp_task_params! {
    ComplexTask {
        size: usize,
        result: *mut f64,
    }
}

zp_task_params! {
    EmptyTask {
        value: *mut u64,
    }
}

// Task functions
zp_define_task_fn!(simple_task_fn, SimpleTask, |params| {
    let mut sum = 0u64;
    for i in 0..params.iterations {
        sum = sum.wrapping_add(i as u64 * 17);
    }
    unsafe { *params.result = sum; }
});

zp_define_task_fn!(complex_task_fn, ComplexTask, |params| {
    // Simulate matrix operations
    let mut result = 0.0;
    for i in 0..params.size {
        for j in 0..params.size {
            result += ((i * j) as f64).sqrt().sin();
        }
    }
    unsafe { *params.result = result; }
});

zp_define_task_fn!(empty_task_fn, EmptyTask, |params| {
    unsafe { *params.value = 1; }
});

// Benchmark simple tasks - small amount
fn bench_simple_small(c: &mut Criterion) {
    let mut group = c.benchmark_group("simple_small_100");
    
    group.bench_function("zero_pool", |b| {
        let pool = zero_pool::new();
        b.iter(|| {
            let mut results = vec![0u64; 100];
            let tasks: Vec<_> = results.iter_mut().map(|result| {
                SimpleTask::new(100, result)
            }).collect();
            
            let batch = zp_submit_batch_uniform!(pool, tasks, simple_task_fn);
            batch.wait();
            black_box(results);
        });
    });
    
    group.bench_function("rayon", |b| {
        b.iter(|| {
            let results: Vec<u64> = (0..100)
                .into_par_iter()
                .map(|_| {
                    let mut sum = 0u64;
                    for i in 0..100 {
                        sum = sum.wrapping_add(i as u64 * 17);
                    }
                    sum
                })
                .collect();
            black_box(results);
        });
    });
    
    group.finish();
}

// Benchmark simple tasks - large amount
fn bench_simple_large(c: &mut Criterion) {
    let mut group = c.benchmark_group("simple_large_10k");
    group.sample_size(50); // Reduce sample size for larger benchmark
    group.measurement_time(std::time::Duration::from_secs(10)); // Increase measurement time
    
    group.bench_function("zero_pool", |b| {
        let pool = zero_pool::new();
        b.iter(|| {
            let mut results = vec![0u64; 10_000];
            let tasks: Vec<_> = results.iter_mut().map(|result| {
                SimpleTask::new(100, result)
            }).collect();
            
            let batch = zp_submit_batch_uniform!(pool, tasks, simple_task_fn);
            batch.wait();
            black_box(results);
        });
    });
    
    group.bench_function("rayon", |b| {
        b.iter(|| {
            let results: Vec<u64> = (0..10_000)
                .into_par_iter()
                .map(|_| {
                    let mut sum = 0u64;
                    for i in 0..100 {
                        sum = sum.wrapping_add(i as u64 * 17);
                    }
                    sum
                })
                .collect();
            black_box(results);
        });
    });
    
    group.finish();
}

// Benchmark complex tasks - small amount
fn bench_complex_small(c: &mut Criterion) {
    let mut group = c.benchmark_group("complex_small_100");
    
    group.bench_function("zero_pool", |b| {
        let pool = zero_pool::new();
        b.iter(|| {
            let mut results = vec![0.0; 100];
            let tasks: Vec<_> = results.iter_mut().map(|result| {
                ComplexTask::new(50, result)
            }).collect();
            
            let batch = zp_submit_batch_uniform!(pool, tasks, complex_task_fn);
            batch.wait();
            black_box(results);
        });
    });
    
    group.bench_function("rayon", |b| {
        b.iter(|| {
            let results: Vec<f64> = (0..100)
                .into_par_iter()
                .map(|_| {
                    let mut result = 0.0;
                    for i in 0..50 {
                        for j in 0..50 {
                            result += ((i * j) as f64).sqrt().sin();
                        }
                    }
                    result
                })
                .collect();
            black_box(results);
        });
    });
    
    group.finish();
}

// Benchmark complex tasks - large amount
fn bench_complex_large(c: &mut Criterion) {
    let mut group = c.benchmark_group("complex_large_1k");
    group.sample_size(20); // Further reduce sample size for complex operations
    group.measurement_time(std::time::Duration::from_secs(15)); // Longer measurement time
    
    group.bench_function("zero_pool", |b| {
        let pool = zero_pool::new();
        b.iter(|| {
            let mut results = vec![0.0; 1000];
            let tasks: Vec<_> = results.iter_mut().map(|result| {
                ComplexTask::new(50, result)
            }).collect();
            
            let batch = zp_submit_batch_uniform!(pool, tasks, complex_task_fn);
            batch.wait();
            black_box(results);
        });
    });
    
    group.bench_function("rayon", |b| {
        b.iter(|| {
            let results: Vec<f64> = (0..1000)
                .into_par_iter()
                .map(|_| {
                    let mut result = 0.0;
                    for i in 0..50 {
                        for j in 0..50 {
                            result += ((i * j) as f64).sqrt().sin();
                        }
                    }
                    result
                })
                .collect();
            black_box(results);
        });
    });
    
    group.finish();
}

// Benchmark mixed workload
fn bench_mixed_large(c: &mut Criterion) {
    let mut group = c.benchmark_group("mixed_large_5k");
    group.sample_size(30); // Reduce sample size
    group.measurement_time(std::time::Duration::from_secs(10)); // Increase measurement time
    
    group.bench_function("zero_pool", |b| {
        let pool = zero_pool::new();
        b.iter(|| {
            let mut simple_results = vec![0u64; 2500];
            let mut complex_results = vec![0.0; 2500];
            
            // Create mixed tasks
            let simple_tasks: Vec<_> = simple_results.iter_mut().map(|result| {
                SimpleTask::new(100, result)
            }).collect();
            
            let complex_tasks: Vec<_> = complex_results.iter_mut().map(|result| {
                ComplexTask::new(30, result)
            }).collect();
            
            // Submit both batches
            let simple_batch = zp_submit_batch_uniform!(pool, simple_tasks, simple_task_fn);
            let complex_batch = zp_submit_batch_uniform!(pool, complex_tasks, complex_task_fn);
            
            // Wait for both
            simple_batch.wait();
            complex_batch.wait();
            
            black_box((simple_results, complex_results));
        });
    });
    
    group.bench_function("rayon", |b| {
        b.iter(|| {
            // Simple tasks
            let simple_results: Vec<u64> = (0..2500)
                .into_par_iter()
                .map(|_| {
                    let mut sum = 0u64;
                    for i in 0..100 {
                        sum = sum.wrapping_add(i as u64 * 17);
                    }
                    sum
                })
                .collect();
            
            // Complex tasks
            let complex_results: Vec<f64> = (0..2500)
                .into_par_iter()
                .map(|_| {
                    let mut result = 0.0;
                    for i in 0..30 {
                        for j in 0..30 {
                            result += ((i * j) as f64).sqrt().sin();
                        }
                    }
                    result
                })
                .collect();
            
            black_box((simple_results, complex_results));
        });
    });
    
    group.finish();
}

// Memory overhead comparison (theoretical)
fn bench_memory_overhead(c: &mut Criterion) {
    let mut group = c.benchmark_group("memory_overhead_per_task");
    
    group.bench_function("zero_pool_24_bytes", |b| {
        b.iter(|| {
            // Zero-pool uses 24 bytes per work item:
            // - 8 bytes for params pointer
            // - 8 bytes for function pointer  
            // - 8 bytes for future Arc pointer
            let overhead_per_task = 24;
            let tasks = 10_000;
            black_box(overhead_per_task * tasks);
        });
    });
    
    group.bench_function("rayon_closure_overhead", |b| {
        b.iter(|| {
            // Rayon has to box closures and maintain work-stealing deques
            // Typical closure + Box + deque overhead is much higher
            // This is a simplified estimate
            let closure_size = 24; // Minimum closure size
            let box_overhead = 16; // Box allocation overhead
            let deque_overhead = 8; // Work-stealing deque pointer
            let overhead_per_task = closure_size + box_overhead + deque_overhead;
            let tasks = 10_000;
            black_box(overhead_per_task * tasks);
        });
    });
    
    group.finish();
}

// Minimal task overhead - submitting empty tasks
fn bench_minimal_overhead(c: &mut Criterion) {
    let mut group = c.benchmark_group("minimal_task_overhead");
    
    group.bench_function("zero_pool", |b| {
        let pool = zero_pool::new();
        b.iter(|| {
            let mut results = vec![0u64; 1000];
            let tasks: Vec<_> = results.iter_mut().map(|result| {
                EmptyTask::new(result)
            }).collect();
            
            let batch = zp_submit_batch_uniform!(pool, tasks, empty_task_fn);
            batch.wait();
            black_box(results);
        });
    });
    
    group.bench_function("rayon", |b| {
        b.iter(|| {
            let results: Vec<u64> = (0..1000)
                .into_par_iter()
                .map(|_| 1u64)
                .collect();
            black_box(results);
        });
    });
    
    group.finish();
}

criterion_group!(
    benches,
    bench_simple_small,
    bench_simple_large,
    bench_complex_small,
    bench_complex_large,
    bench_mixed_large,
    bench_memory_overhead,
    bench_minimal_overhead
);

criterion_main!(benches);