use std::{
    hint::black_box,
    time::{Duration, Instant},
};
use zero_pool::{ZeroPool, global_pool, zp_define_task_fn, zp_write};

// Define task parameter structures
struct SimpleTask {
    iterations: usize,
    result: *mut u64,
}
struct ComplexTask {
    size: usize,
    result: *mut f64,
}

// Define task functions
zp_define_task_fn!(simple_task_fn, SimpleTask, |params| {
    let mut sum = 0u64;
    for i in 0..params.iterations {
        sum = sum.wrapping_add(i as u64 * 17);
    }
    zp_write!(params.result, sum);
});

zp_define_task_fn!(simple_cpu_task, SimpleTask, |params| {
    let mut sum = 0u64;
    for i in 0..params.iterations {
        sum = sum.wrapping_add(i as u64 * 17 + 23);
    }
    zp_write!(params.result, sum);
});

zp_define_task_fn!(complex_task_fn, ComplexTask, |params| {
    let mut result = 0.0;
    for i in 0..params.size {
        for j in 0..params.size {
            result += ((i * j) as f64).sqrt().sin();
        }
    }
    zp_write!(params.result, result);
});

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
    println!("Basic test result: {}", result);
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
    let task_count = 10_000;
    let mut results = vec![0u64; task_count];

    println!("Starting {} simple tasks...", task_count);
    let start = Instant::now();

    let tasks: Vec<_> = results
        .iter_mut()
        .map(|result| SimpleTask {
            iterations: 100,
            result,
        })
        .collect();

    let batch = pool.submit_batch_uniform(simple_cpu_task, &tasks);
    batch.wait();

    let duration = start.elapsed();
    println!("Completed {} tasks in {:?}", task_count, duration);

    let completed_count = results.iter().filter(|&&r| r != 0).count();
    println!("Completed tasks: {} / {}", completed_count, task_count);
    assert_eq!(completed_count, task_count, "Not all tasks completed");
}

#[test]
fn test_empty_batch_submission() {
    let pool = ZeroPool::new();

    println!("Testing empty batch submissions...");

    let empty_tasks: Vec<SimpleTask> = Vec::new();
    let empty_batch = pool.submit_batch_uniform(simple_cpu_task, &empty_tasks);
    assert!(empty_batch.is_complete());
    empty_batch.wait();

    println!("Empty batch tests completed successfully");
}

#[test]
fn test_single_worker_behavior() {
    // Important to test single-threaded execution path
    let pool = ZeroPool::with_workers(1);

    println!("Testing single worker pool behavior...");

    let mut results = [0u64; 5];
    let start = Instant::now();

    let tasks: Vec<_> = results
        .iter_mut()
        .enumerate()
        .map(|(i, result)| SimpleTask {
            iterations: 10000 + i * 1000,
            result,
        })
        .collect();

    let batch = pool.submit_batch_uniform(simple_cpu_task, &tasks);
    batch.wait();

    let duration = start.elapsed();

    for (i, &result) in results.iter().enumerate() {
        assert_ne!(result, 0, "Task {} did not complete", i);
    }

    println!("Single worker test completed in {:?}", duration);
}

#[test]
fn test_different_worker_counts() {
    // Test pool behavior with different worker counts
    for worker_count in [1, 2, 4, 8] {
        println!("Testing with {} workers", worker_count);

        let pool = ZeroPool::with_workers(worker_count);
        let task_count = worker_count * 10;
        let mut results = vec![0u64; task_count];

        let tasks: Vec<_> = results
            .iter_mut()
            .map(|result| SimpleTask {
                iterations: 1000,
                result,
            })
            .collect();

        let start = Instant::now();
        let batch = pool.submit_batch_uniform(simple_cpu_task, &tasks);
        batch.wait();
        let duration = start.elapsed();

        let completed = results.iter().filter(|&&r| r != 0).count();
        assert_eq!(completed, task_count);

        println!(
            "  {} workers completed {} tasks in {:?}",
            worker_count, task_count, duration
        );
    }
}

#[test]
fn test_shutdown_and_cleanup() {
    println!("Testing shutdown and cleanup behavior...");

    let mut final_results = vec![0u64; 100];
    let completed_before_drop;

    {
        let pool = ZeroPool::new();

        let tasks: Vec<_> = final_results
            .iter_mut()
            .map(|result| SimpleTask {
                iterations: 1000,
                result,
            })
            .collect();

        let batch = pool.submit_batch_uniform(simple_cpu_task, &tasks);
        batch.wait();

        completed_before_drop = final_results.iter().filter(|&&r| r != 0).count();
        println!("Tasks completed before drop: {}", completed_before_drop);
    }

    println!("Pool dropped successfully");
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
        println!("New pool works correctly after previous cleanup");
    }
}

#[test]
fn test_rapid_pool_creation() {
    // Tests rapid pool creation and destruction
    for iteration in 0..5 {
        println!("Rapid pool iteration {}", iteration);

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

        std::thread::sleep(Duration::from_millis(10));
    }
    println!("Rapid pool creation test completed");
}

#[test]
fn test_wait_timeout() {
    let pool = ZeroPool::new();

    // Test that timeout works for quick tasks
    let mut result = 0u64;
    let params = SimpleTask {
        iterations: 100,
        result: &mut result,
    };
    let future = pool.submit_task(simple_cpu_task, &params);

    let start = Instant::now();
    let completed = future.wait_timeout(Duration::from_secs(5));
    let duration = start.elapsed();

    assert!(completed, "Task should complete within timeout");
    assert_ne!(result, 0);
    println!("Task completed in {:?}", duration);

    // Test empty batch with timeout
    let empty_tasks: Vec<SimpleTask> = Vec::new();
    let batch = pool.submit_batch_uniform(simple_cpu_task, &empty_tasks);

    let start = Instant::now();
    let completed = batch.wait_timeout(Duration::from_millis(100));
    let duration = start.elapsed();

    assert!(completed);
    assert!(
        duration < Duration::from_millis(50),
        "Empty batch should complete immediately"
    );
}

#[test]
fn test_stress_rapid_batches() {
    let pool = ZeroPool::new();

    for batch_num in 0..10 {
        let mut results = vec![0u64; 100];
        let tasks: Vec<_> = results
            .iter_mut()
            .enumerate()
            .map(|(i, result)| SimpleTask {
                iterations: 200 + i * 10,
                result,
            })
            .collect();

        let start = Instant::now();
        let batch = pool.submit_batch_uniform(simple_cpu_task, &tasks);
        let completed = batch.wait_timeout(Duration::from_secs(10));
        let duration = start.elapsed();

        assert!(completed, "Batch {} timed out", batch_num);

        let completed_count = results.iter().filter(|&&r| r != 0).count();
        assert_eq!(completed_count, 100);

        println!("Stress batch {} completed in {:?}", batch_num, duration);
    }
}

#[test]
fn test_benchmark_simulation() {
    // Simulates what criterion does - multiple iterations with the same pool
    let pool = ZeroPool::new();

    for iteration in 0..20 {
        let mut results = vec![0u64; 100];
        let tasks: Vec<_> = results
            .iter_mut()
            .map(|result| SimpleTask {
                iterations: 100,
                result,
            })
            .collect();

        let batch = pool.submit_batch_uniform(simple_task_fn, &tasks);
        let completed = batch.wait_timeout(Duration::from_secs(5));

        assert!(completed, "Iteration {} timed out", iteration);

        let completed_count = results.iter().filter(|&&r| r != 0).count();
        assert_eq!(
            completed_count, 100,
            "Not all tasks completed in iteration {}",
            iteration
        );

        black_box(results);
    }
    println!("Benchmark simulation completed successfully");
}

#[test]
fn test_memory_pressure() {
    let pool = ZeroPool::new();
    let mut all_results = Vec::new();

    for iteration in 0..20 {
        let mut results = vec![0u64; 1000];
        let tasks: Vec<_> = results
            .iter_mut()
            .map(|result| SimpleTask {
                iterations: 500,
                result,
            })
            .collect();

        let batch = pool.submit_batch_uniform(simple_task_fn, &tasks);
        let completed = batch.wait_timeout(Duration::from_secs(15));

        assert!(
            completed,
            "Memory pressure iteration {} timed out",
            iteration
        );

        all_results.push(results);
        black_box(&all_results);

        println!(
            "Memory pressure iteration {} completed, total: {} MB",
            iteration,
            all_results.len() * 1000 * 8 / 1_000_000
        );
    }
}

#[test]
fn test_complex_workload_scaling() {
    // Test how complex tasks scale with worker count
    for workers in [1, 2, 4] {
        let pool = ZeroPool::with_workers(workers);
        let mut results = [0.0; 20];

        let params: Vec<_> = results
            .iter_mut()
            .map(|result| ComplexTask { size: 50, result })
            .collect();

        let start = Instant::now();
        let batch = pool.submit_batch_uniform(complex_task_fn, &params);
        batch.wait();
        let duration = start.elapsed();

        let completed = results.iter().filter(|&&r| r != 0.0).count();
        assert_eq!(completed, 20);

        println!("Complex tasks with {} workers: {:?}", workers, duration);
    }
}
