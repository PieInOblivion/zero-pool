use std::{
    hint::black_box,
    time::{Duration, Instant},
};
use zero_pool::{
    TaskFnPointer, TaskParamPointer, ZeroPool, zp_define_task_fn, zp_submit_batch_mixed,
    zp_task_params, zp_write,
};

// Define task parameter structures
zp_task_params! {
    SimpleTask {
        iterations: usize,
        result: *mut u64,
    }
}

zp_task_params! {
    PrimeTask {
        number: u64,
        result: *mut bool,
    }
}

zp_task_params! {
    MatrixTask {
        size: usize,
        result: *mut f64,
    }
}

zp_task_params! {
    SortTask {
        size: usize,
        data: *mut Vec<i32>,
    }
}

zp_task_params! {
    ComplexTask {
        size: usize,
        result: *mut f64,
    }
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

zp_define_task_fn!(is_prime_task, PrimeTask, |params| {
    let n = params.number;
    let is_prime = if n < 2 {
        false
    } else if n == 2 {
        true
    } else if n % 2 == 0 {
        false
    } else {
        let sqrt_n = (n as f64).sqrt() as u64;
        let mut prime = true;
        for i in (3..=sqrt_n).step_by(2) {
            if n % i == 0 {
                prime = false;
                break;
            }
        }
        prime
    };
    zp_write!(params.result, is_prime);
});

zp_define_task_fn!(matrix_multiply_task, MatrixTask, |params| {
    let size = params.size;
    let mut a = vec![vec![1.0f64; size]; size];
    let mut b = vec![vec![2.0f64; size]; size];
    let mut c = vec![vec![0.0f64; size]; size];

    for i in 0..size {
        for j in 0..size {
            a[i][j] = (i * j + 1) as f64;
            b[i][j] = (i + j + 1) as f64;
        }
    }

    for i in 0..size {
        for j in 0..size {
            for k in 0..size {
                c[i][j] += a[i][k] * b[k][j];
            }
        }
    }

    let mut diagonal_sum = 0.0;
    for i in 0..size {
        diagonal_sum += c[i][i];
    }
    zp_write!(params.result, diagonal_sum);
});

zp_define_task_fn!(sort_data_task, SortTask, |params| {
    let data = unsafe { &mut *params.data };
    data.clear();
    for i in 0..params.size {
        data.push(((i * 17 + 23) % 1000) as i32);
    }

    let len = data.len();
    for i in 0..len {
        for j in 0..len - 1 - i {
            if data[j] > data[j + 1] {
                data.swap(j, j + 1);
            }
        }
    }
});

#[test]
fn test_basic_functionality() {
    let pool = ZeroPool::new();
    let mut result = 0u64;

    let task = SimpleTask::new(1000, &mut result);
    let future = pool.submit_task(simple_cpu_task, &task);
    future.wait();

    assert_ne!(result, 0);
    println!("Basic test result: {}", result);
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
        .map(|result| SimpleTask::new(100, result))
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
fn test_mixed_workload() {
    let pool = ZeroPool::new();

    let mut simple_result = 0u64;
    let simple_task = SimpleTask::new(50000, &mut simple_result);

    let mut prime_result = false;
    let prime_task = PrimeTask::new(982451653, &mut prime_result);

    let mut matrix_result = 0.0f64;
    let matrix_task = MatrixTask::new(80, &mut matrix_result);

    let mut sort_data = Vec::new();
    let sort_task = SortTask::new(5000, &mut sort_data);

    println!("Running mixed workload...");
    let start = Instant::now();

    let batch = zp_submit_batch_mixed!(
        pool,
        [
            (&simple_task, simple_cpu_task),
            (&prime_task, is_prime_task),
            (&matrix_task, matrix_multiply_task),
            (&sort_task, sort_data_task),
        ]
    );

    batch.wait();

    let duration = start.elapsed();
    println!("Mixed workload completed in {:?}", duration);

    assert_ne!(simple_result, 0);
    assert_ne!(matrix_result, 0.0);
    assert!(!sort_data.is_empty());
    println!("Simple task result: {}", simple_result);
    println!("Prime check result: {}", prime_result);
    println!("Matrix result: {}", matrix_result);
    println!("Sort result length: {}", sort_data.len());
}

#[test]
fn test_empty_batch_submission() {
    let pool = ZeroPool::new();

    println!("Testing empty batch submissions...");

    let empty_tasks: Vec<SimpleTask> = Vec::new();
    let empty_batch = pool.submit_batch_uniform(simple_cpu_task, &empty_tasks);
    assert!(empty_batch.is_complete());
    empty_batch.wait();

    let empty_mixed = zp_submit_batch_mixed!(pool, []);
    assert!(empty_mixed.is_complete());
    empty_mixed.wait();

    println!("Empty batch tests completed successfully");
}

#[test]
fn test_single_worker_behavior() {
    // Important to test single-threaded execution path
    let pool = ZeroPool::with_workers(1);

    println!("Testing single worker pool behavior...");

    let mut results = vec![0u64; 5];
    let start = Instant::now();

    let tasks: Vec<_> = results
        .iter_mut()
        .enumerate()
        .map(|(i, result)| SimpleTask::new(10000 + i * 1000, result))
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
            .map(|result| SimpleTask::new(1000, result))
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
            .map(|result| SimpleTask::new(1000, result))
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
        let task = SimpleTask::new(100, &mut test_result);
        let future = pool2.submit_task(simple_cpu_task, &task);
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
        let task = SimpleTask::new(500, &mut result);

        let future = pool.submit_task(simple_cpu_task, &task);
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
    let task = SimpleTask::new(100, &mut result);
    let future = pool.submit_task(simple_cpu_task, &task);

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
            .map(|(i, result)| SimpleTask::new(200 + i * 10, result))
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
            .map(|result| SimpleTask::new(100, result))
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
            .map(|result| SimpleTask::new(500, result))
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
fn test_optimized_uniform_batch() {
    let pool = ZeroPool::new();
    let task_count = 1000;
    let mut results = vec![0u64; task_count];

    let tasks: Vec<_> = results
        .iter_mut()
        .map(|result| SimpleTask::new(100, result))
        .collect();

    let tasks_converted = zero_pool::uniform_tasks_to_pointers(simple_task_fn, &tasks);

    let start = Instant::now();
    let batch = pool.submit_raw_task_batch(&tasks_converted);
    batch.wait();
    let duration = start.elapsed();

    println!("Optimized uniform batch completed in {:?}", duration);

    let completed_count = results.iter().filter(|&&r| r != 0).count();
    assert_eq!(completed_count, task_count);
}

#[test]
fn test_optimized_api_equivalence() {
    let pool = ZeroPool::new();
    let task_count = 500;

    // Test normal API
    let mut normal_results = vec![0u64; task_count];
    let normal_tasks: Vec<_> = normal_results
        .iter_mut()
        .map(|result| SimpleTask::new(123, result))
        .collect();

    let normal_batch = pool.submit_batch_uniform(simple_task_fn, &normal_tasks);
    normal_batch.wait();

    // Test optimized API
    let mut optimized_results = vec![0u64; task_count];
    let optimized_tasks: Vec<_> = optimized_results
        .iter_mut()
        .map(|result| SimpleTask::new(123, result))
        .collect();

    let tasks_converted = zero_pool::uniform_tasks_to_pointers(simple_task_fn, &optimized_tasks);
    let optimized_batch = pool.submit_raw_task_batch(&tasks_converted);
    optimized_batch.wait();

    // Verify results are identical
    for (i, (&normal, &optimized)) in normal_results
        .iter()
        .zip(optimized_results.iter())
        .enumerate()
    {
        assert_eq!(normal, optimized, "Result mismatch at index {}", i);
    }
}

#[test]
fn test_optimized_mixed_batch() {
    let pool = ZeroPool::new();

    let mut simple_result = 0u64;
    let simple_task = SimpleTask::new(50000, &mut simple_result);

    let mut prime_result = false;
    let prime_task = PrimeTask::new(982451653, &mut prime_result);

    let mixed_tasks = vec![
        (
            simple_cpu_task as TaskFnPointer,
            &simple_task as *const _ as TaskParamPointer,
        ),
        (
            is_prime_task as TaskFnPointer,
            &prime_task as *const _ as TaskParamPointer,
        ),
    ];

    let batch = pool.submit_raw_task_batch(&mixed_tasks);
    batch.wait();

    assert_ne!(simple_result, 0);
    println!(
        "Optimized mixed batch - Simple: {}, Prime: {}",
        simple_result, prime_result
    );
}

#[test]
fn test_optimized_reuse_pattern() {
    // Tests reusing converted task pointers across multiple submissions
    let pool = ZeroPool::new();
    let task_count = 100;

    let mut results = vec![0u64; task_count];
    let tasks: Vec<_> = results
        .iter_mut()
        .map(|result| SimpleTask::new(500, result))
        .collect();

    let tasks_converted = zero_pool::uniform_tasks_to_pointers(simple_task_fn, &tasks);

    for iteration in 0..5 {
        for result in results.iter_mut() {
            *result = 0;
        }

        let batch = pool.submit_raw_task_batch(&tasks_converted);
        batch.wait();

        let completed_count = results.iter().filter(|&&r| r != 0).count();
        assert_eq!(
            completed_count, task_count,
            "Reuse iteration {} failed",
            iteration
        );

        black_box(results.clone());
    }
    println!("Task pointer reuse pattern test completed");
}

#[test]
fn test_complex_workload_scaling() {
    // Test how complex tasks scale with worker count
    for workers in [1, 2, 4] {
        let pool = ZeroPool::with_workers(workers);
        let mut results = vec![0.0; 20];

        let tasks: Vec<_> = results
            .iter_mut()
            .map(|result| ComplexTask::new(50, result))
            .collect();

        let start = Instant::now();
        let batch = pool.submit_batch_uniform(complex_task_fn, &tasks);
        batch.wait();
        let duration = start.elapsed();

        let completed = results.iter().filter(|&&r| r != 0.0).count();
        assert_eq!(completed, 20);

        println!("Complex tasks with {} workers: {:?}", workers, duration);
    }
}
