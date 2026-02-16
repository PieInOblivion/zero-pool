use std::{
    hint::black_box,
    num::NonZeroUsize,
    time::{Duration, Instant},
};
use zero_pool::{ZeroPool, global_pool};

// Define task parameter structures
struct SimpleTask {
    iterations: usize,
    result: *mut u64,
}
struct ComplexTask {
    size: usize,
    result: *mut u64,
}

// Define task functions
fn simple_task_fn(params: &SimpleTask) {
    let mut sum = 0u64;
    for i in 0..params.iterations {
        sum = sum.wrapping_add(i as u64 * 17);
    }
    unsafe {
        *params.result = sum;
    }
}

fn simple_cpu_task(params: &SimpleTask) {
    let mut sum = 0u64;
    for i in 0..params.iterations {
        sum = sum.wrapping_add(i as u64 * 17 + 23);
    }
    unsafe {
        *params.result = sum;
    }
}

fn complex_task_fn(params: &ComplexTask) {
    let mut result = 0u64;
    for i in 0..params.size {
        for j in 0..params.size {
            let val = (i * j) as u64;
            result = result.wrapping_add(val.wrapping_mul(val));
        }
    }
    unsafe {
        *params.result = result;
    }
}

// Tests
#[test]
fn test_basic_functionality() {
    let pool = ZeroPool::new();
    let mut result = 0u64;

    let params = SimpleTask {
        iterations: 1000,
        result: &mut result,
    };
    let future = pool.submit_task(simple_cpu_task, &params);
    future.wait();

    assert_ne!(result, 0);
}

#[test]
fn test_global_pool_usage() {
    let pool = global_pool();
    let mut result = 0u64;

    let params = SimpleTask {
        iterations: 1000,
        result: &mut result,
    };

    let future = pool.submit_task(simple_cpu_task, &params);
    future.wait();

    assert_ne!(result, 0);
    assert!(
        std::ptr::eq(pool, global_pool()),
        "Global pool should be a singleton"
    );
}

#[test]
fn test_massive_simple_tasks() {
    let pool = ZeroPool::new();
    let count = 200;
    let mut results = vec![0u64; count];

    let tasks: Vec<_> = results
        .iter_mut()
        .map(|result| SimpleTask {
            iterations: 100,
            result,
        })
        .collect();

    let batch = pool.submit_batch(simple_cpu_task, &tasks);
    batch.wait();

    let completed_count = results.iter().filter(|&&r| r != 0).count();
    assert_eq!(completed_count, count, "Not all tasks completed");
}

#[test]
fn test_empty_batch_submission() {
    let pool = ZeroPool::new();
    let empty_tasks: Vec<SimpleTask> = Vec::new();
    let empty_batch = pool.submit_batch(simple_cpu_task, &empty_tasks);

    assert!(empty_batch.is_complete());
    empty_batch.wait();
}

#[test]
fn test_single_worker_behavior() {
    let pool = ZeroPool::with_workers(NonZeroUsize::MIN);
    let mut results = [0u64; 5];
    let base_iter = 10_000;

    let tasks: Vec<_> = results
        .iter_mut()
        .enumerate()
        .map(|(i, result)| SimpleTask {
            iterations: base_iter + i * 100,
            result,
        })
        .collect();

    let batch = pool.submit_batch(simple_cpu_task, &tasks);
    batch.wait();

    for (i, &result) in results.iter().enumerate() {
        assert_ne!(result, 0, "Task {} did not complete", i);
    }
}

#[test]
fn test_different_worker_counts() {
    let worker_counts = [1, 2, 4];

    for worker_count in worker_counts {
        let pool = ZeroPool::with_workers(NonZeroUsize::new(worker_count).unwrap());
        let task_count = worker_count * 10;
        let mut results = vec![0u64; task_count];

        let tasks: Vec<_> = results
            .iter_mut()
            .map(|result| SimpleTask {
                iterations: 100,
                result,
            })
            .collect();

        let batch = pool.submit_batch(simple_cpu_task, &tasks);
        batch.wait();

        let completed = results.iter().filter(|&&r| r != 0).count();
        assert_eq!(completed, task_count);
    }
}

#[test]
fn test_shutdown_and_cleanup() {
    let mut final_results = vec![0u64; 100];
    let completed_before_drop;
    let iterations = 20;

    {
        let pool = ZeroPool::new();

        let tasks: Vec<_> = final_results
            .iter_mut()
            .map(|result| SimpleTask { iterations, result })
            .collect();

        let batch = pool.submit_batch(simple_cpu_task, &tasks);
        batch.wait();

        completed_before_drop = final_results.iter().filter(|&&r| r != 0).count();
    }

    assert_eq!(
        completed_before_drop, 100,
        "Not all tasks completed before shutdown"
    );

    // Test that a new pool can be created after the old one was dropped
    {
        let pool2 = ZeroPool::new();
        let mut test_result = 0u64;
        let params = SimpleTask {
            iterations: 100,
            result: &mut test_result,
        };
        let future = pool2.submit_task(simple_cpu_task, &params);
        future.wait();
        assert_ne!(test_result, 0);
    }
}

#[test]
fn test_rapid_pool_creation() {
    for _ in 0..5 {
        let pool = ZeroPool::new();
        let mut result = 0u64;
        let params = SimpleTask {
            iterations: 500,
            result: &mut result,
        };

        let future = pool.submit_task(simple_cpu_task, &params);
        future.wait();

        assert_ne!(result, 0);
        drop(pool);
    }
}

#[test]
fn test_wait_timeout() {
    let pool = ZeroPool::new();

    let mut result = 0u64;
    let params = SimpleTask {
        iterations: 100,
        result: &mut result,
    };
    let future = pool.submit_task(simple_cpu_task, &params);

    let completed = future.wait_timeout(Duration::from_secs(5));

    assert!(completed, "Task should complete within timeout");
    assert_ne!(result, 0);

    // Test empty batch with timeout
    let empty_tasks: Vec<SimpleTask> = Vec::new();
    let batch = pool.submit_batch(simple_cpu_task, &empty_tasks);

    let start = Instant::now();
    let completed = batch.wait_timeout(Duration::from_millis(100));
    let duration = start.elapsed();

    assert!(completed);
    assert!(
        duration < Duration::from_secs(1),
        "Empty batch should complete immediately"
    );
}

#[test]
fn test_stress_rapid_batches() {
    let pool = ZeroPool::new();
    let iterations = 2;
    let batch_size = 10;

    for batch_num in 0..iterations {
        let mut results = vec![0u64; batch_size];
        let tasks: Vec<_> = results
            .iter_mut()
            .enumerate()
            .map(|(i, result)| SimpleTask {
                iterations: 100 + i,
                result,
            })
            .collect();

        let batch = pool.submit_batch(simple_cpu_task, &tasks);
        let completed = batch.wait_timeout(Duration::from_secs(10));

        assert!(completed, "Batch {} timed out", batch_num);

        let completed_count = results.iter().filter(|&&r| r != 0).count();
        assert_eq!(completed_count, batch_size);
    }
}

#[test]
fn test_benchmark_simulation() {
    let pool = ZeroPool::new();
    let iterations = 10;

    for iteration in 0..iterations {
        let mut results = vec![0u64; 100];
        let tasks: Vec<_> = results
            .iter_mut()
            .map(|result| SimpleTask {
                iterations: 100,
                result,
            })
            .collect();

        let batch = pool.submit_batch(simple_task_fn, &tasks);
        let completed = batch.wait_timeout(Duration::from_secs(5));

        assert!(completed, "Iteration {} timed out", iteration);

        let completed_count = results.iter().filter(|&&r| r != 0).count();
        assert_eq!(completed_count, 100);

        black_box(results);
    }
}

#[test]
fn test_complex_workload_scaling() {
    let worker_counts = [1, 2, 4];
    let size = 10;

    for workers in worker_counts {
        let pool = ZeroPool::with_workers(NonZeroUsize::new(workers).unwrap());
        let mut results = [0u64; 10];

        let params: Vec<_> = results
            .iter_mut()
            .map(|result| ComplexTask { size, result })
            .collect();

        let batch = pool.submit_batch(complex_task_fn, &params);
        batch.wait();

        let completed = results.iter().filter(|&&r| r != 0).count();
        assert_eq!(completed, 10);
    }
}

#[test]
fn test_reclaim_trigger() {
    // This test ensures we hit the reclaim threshold (256 batches)
    let pool = ZeroPool::new();
    let mut result = 0u64;
    let params = SimpleTask {
        iterations: 0,
        result: &mut result,
    };

    for _ in 0..300 {
        let future = pool.submit_task(simple_cpu_task, &params);
        future.wait();
    }
}
