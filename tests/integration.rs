use std::{
    hint::black_box,
    time::{Duration, Instant},
};
use zero_pool::{
    self, zp_define_task_fn, zp_submit_batch_mixed, zp_task_params, zp_write, TaskFnPointer, TaskParamPointer, WorkItem
};

// define task parameter structures using the safe macro
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
    HashTask {
        data: *const [u8; 1024],
        iterations: usize,
        result: *mut u64,
    }
}

zp_task_params! {
    DebugTask {
        id: usize,
        work_amount: usize,
        result: *mut u64,
    }
}

zp_task_params! {
    ComplexTask {
        size: usize,
        result: *mut f64,
    }
}

zp_define_task_fn!(simple_task_fn, SimpleTask, |params| {
    let mut sum = 0u64;
    for i in 0..params.iterations {
        sum = sum.wrapping_add(i as u64 * 17);
    }
    zp_write!(params.result, sum);
});

zp_define_task_fn!(complex_task_fn, ComplexTask, |params| {
    // simulate matrix operations
    let mut result = 0.0;
    for i in 0..params.size {
        for j in 0..params.size {
            result += ((i * j) as f64).sqrt().sin();
        }
    }
    zp_write!(params.result, result);
});

zp_define_task_fn!(debug_task_fn, DebugTask, |params| {
    println!(
        "  Task {} starting with work_amount {}",
        params.id, params.work_amount
    );

    let mut sum = 0u64;
    for i in 0..params.work_amount {
        sum = sum.wrapping_add(i as u64 * 17);
    }
    zp_write!(params.result, sum);

    println!("  Task {} completed with result {}", params.id, sum);
});

// define task functions using the safe macro
zp_define_task_fn!(simple_cpu_task, SimpleTask, |params| {
    let mut sum = 0u64;
    for i in 0..params.iterations {
        sum = sum.wrapping_add(i as u64 * 17 + 23);
    }
    zp_write!(params.result, sum);
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

    // Create and multiply two matrices
    let mut a = vec![vec![1.0f64; size]; size];
    let mut b = vec![vec![2.0f64; size]; size];
    let mut c = vec![vec![0.0f64; size]; size];

    // Initialise with some patterns
    for i in 0..size {
        for j in 0..size {
            a[i][j] = (i * j + 1) as f64;
            b[i][j] = (i + j + 1) as f64;
        }
    }

    // Matrix multiplication
    for i in 0..size {
        for j in 0..size {
            for k in 0..size {
                c[i][j] += a[i][k] * b[k][j];
            }
        }
    }

    // Return sum of diagonal
    let mut diagonal_sum = 0.0;
    for i in 0..size {
        diagonal_sum += c[i][i];
    }

    zp_write!(params.result, diagonal_sum);
});

zp_define_task_fn!(sort_data_task, SortTask, |params| {
    let data = unsafe { &mut *params.data };

    // Fill with pseudo-random data
    data.clear();
    for i in 0..params.size {
        data.push(((i * 17 + 23) % 1000) as i32);
    }

    // Bubble sort (intentionally inefficient for CPU load)
    let len = data.len();
    for i in 0..len {
        for j in 0..len - 1 - i {
            if data[j] > data[j + 1] {
                data.swap(j, j + 1);
            }
        }
    }
});

zp_define_task_fn!(hash_intensive_task, HashTask, |params| {
    let data = unsafe { &*params.data };
    let mut hash = 0u64;

    for _ in 0..params.iterations {
        for &byte in data.iter() {
            hash = hash.wrapping_mul(31).wrapping_add(byte as u64);
        }
    }

    zp_write!(params.result, hash);
});

#[test]
fn test_basic_functionality() {
    let pool = zero_pool::new();
    let mut result = 0u64;

    let task = SimpleTask::new(1000, &mut result);
    let future = pool.submit_task(&task, simple_cpu_task);
    future.wait();

    assert_ne!(result, 0);
    println!("Basic test result: {}", result);
}

#[test]
fn test_massive_simple_tasks() {
    let pool = zero_pool::new();
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

    // Verify all tasks completed
    let completed_count = results.iter().filter(|&&r| r != 0).count();
    println!("Completed tasks: {} / {}", completed_count, task_count);
    assert_eq!(completed_count, task_count, "Not all tasks completed");
}

#[test]
fn test_mixed_workload() {
    let pool = zero_pool::new();

    // Prepare different types of tasks
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
    let pool = zero_pool::new();

    println!("Testing empty batch submissions...");

    // Test empty uniform batch
    let empty_tasks: Vec<SimpleTask> = Vec::new();
    let empty_batch = pool.submit_batch_uniform(simple_cpu_task, &empty_tasks);

    // Should complete immediately
    assert!(empty_batch.is_complete());
    empty_batch.wait();

    // Test empty mixed batch
    let empty_mixed = zp_submit_batch_mixed!(pool, []);
    assert!(empty_mixed.is_complete());
    empty_mixed.wait();

    println!("Empty batch tests completed successfully");
}

#[test]
fn test_single_worker_behavior() {
    let pool = zero_pool::with_workers(1);

    assert_eq!(pool.worker_count(), 1);
    println!("Testing single worker pool behavior...");

    let mut results = vec![0u64; 5];
    let start = Instant::now();

    // Submit tasks that should execute sequentially
    let tasks: Vec<_> = results
        .iter_mut()
        .enumerate()
        .map(|(i, result)| {
            // Different work amounts
            SimpleTask::new(10000 + i * 1000, result)
        })
        .collect();

    let batch = pool.submit_batch_uniform(simple_cpu_task, &tasks);

    // Check queue lengths while work is potentially pending
    println!("Total pending: {}", pool.total_pending());

    batch.wait();
    let duration = start.elapsed();

    // Verify all tasks completed
    for (i, &result) in results.iter().enumerate() {
        assert_ne!(result, 0, "Task {} did not complete", i);
    }

    // After completion, queues should be empty
    assert_eq!(pool.total_pending(), 0);
    println!("Single worker test completed in {:?}", duration);
}

#[test]
fn test_shutdown_and_cleanup() {
    println!("Testing shutdown and cleanup behavior...");

    let mut final_results = vec![0u64; 100];
    let completed_before_drop;

    {
        let pool = zero_pool::new();
        println!("Pool created with {} workers", pool.worker_count());

        // Submit a bunch of work
        let tasks: Vec<_> = final_results
            .iter_mut()
            .map(|result| SimpleTask::new(1000, result))
            .collect();

        let batch = pool.submit_batch_uniform(simple_cpu_task, &tasks);

        // Check pool stats
        println!("Total pending before wait: {}", pool.total_pending());

        // Wait for all work to complete
        batch.wait();

        // Verify work completed
        completed_before_drop = final_results.iter().filter(|&&r| r != 0).count();
        println!("Tasks completed before drop: {}", completed_before_drop);

        // Pool should be clean
        assert_eq!(pool.total_pending(), 0);

        // Pool drops here, testing cleanup
    }

    println!("Pool dropped successfully");

    // Verify all work actually completed
    assert_eq!(
        completed_before_drop, 100,
        "Not all tasks completed before shutdown"
    );

    // Test that a new pool can be created after the old one was dropped
    {
        let pool2 = zero_pool::new();
        let mut test_result = 0u64;
        let task = SimpleTask::new(100, &mut test_result);
        let future = pool2.submit_task(&task, simple_cpu_task);
        future.wait();
        assert_ne!(test_result, 0);
        println!("New pool works correctly after previous cleanup");
    }
}

#[test]
fn test_pool_statistics() {
    let pool = zero_pool::with_workers(4);

    assert_eq!(pool.worker_count(), 4);
    // With MPMC: single shared queue instead of per-worker queues
    assert_eq!(pool.total_pending(), 0);

    // Submit some work to see statistics change
    let mut results = vec![0u64; 20];
    let tasks: Vec<_> = results
        .iter_mut()
        .map(|result| {
            // Longer running tasks
            SimpleTask::new(50000, result)
        })
        .collect();

    let _batch = pool.submit_batch_uniform(simple_cpu_task, &tasks);

    // Check that work was queued
    let total_pending = pool.total_pending();

    println!("Total pending after submission: {}", total_pending);

    // The queue should have some work (though workers may have started processing)
    println!("Queue has work: {}", total_pending > 0);

    // Wait a bit for work to start
    std::thread::sleep(std::time::Duration::from_millis(10));

    println!("Final total pending: {}", pool.total_pending());
}

#[test]
fn debug_test_single_task() {
    println!("Creating pool...");
    let pool = zero_pool::new();
    println!("Pool created with {} workers", pool.worker_count());

    let mut result = 0u64;
    let task = DebugTask::new(1, 1000, &mut result);

    println!("Submitting single task...");
    let future = pool.submit_task(&task, debug_task_fn);

    println!("Waiting for task completion...");
    let start = Instant::now();
    future.wait();
    let duration = start.elapsed();

    println!("Task completed in {:?}, result: {}", duration, result);
    assert_ne!(result, 0);

    println!("Dropping pool...");
    drop(pool);
    println!("Pool dropped successfully");
}

#[test]
fn debug_test_small_batch() {
    println!("Creating pool...");
    let pool = zero_pool::new();
    println!("Pool created with {} workers", pool.worker_count());

    let task_count = 5;
    let mut results = vec![0u64; task_count];

    println!("Creating {} tasks...", task_count);
    let tasks: Vec<_> = results
        .iter_mut()
        .enumerate()
        .map(|(i, result)| {
            println!("  Creating task {}", i);
            DebugTask::new(i, 1000, result)
        })
        .collect();

    println!("Submitting batch...");
    let start = Instant::now();
    let batch = pool.submit_batch_uniform(debug_task_fn, &tasks);
    println!("Batch submitted in {:?}", start.elapsed());

    println!("Waiting for batch completion...");
    let wait_start = Instant::now();
    batch.wait();
    let wait_duration = wait_start.elapsed();

    println!("Batch completed in {:?}", wait_duration);

    // Verify results
    for (i, &result) in results.iter().enumerate() {
        println!("Task {} result: {}", i, result);
        assert_ne!(result, 0, "Task {} did not complete", i);
    }

    println!("Dropping pool...");
    drop(pool);
    println!("Pool dropped successfully");
}

#[test]
fn debug_test_rapid_pools() {
    for iteration in 0..3 {
        println!("--- Iteration {} ---", iteration);

        println!("Creating pool...");
        let pool = zero_pool::new();

        let mut result = 0u64;
        let task = DebugTask::new(iteration, 500, &mut result);

        println!("Submitting task...");
        let future = pool.submit_task(&task, debug_task_fn);

        println!("Waiting...");
        future.wait();

        println!("Task completed, result: {}", result);
        assert_ne!(result, 0);

        println!("Dropping pool...");
        drop(pool);
        println!("Pool dropped");

        // Small delay to see if timing matters
        std::thread::sleep(Duration::from_millis(10));
    }

    println!("Rapid pool test completed");
}

#[test]
fn debug_test_timeout_behavior() {
    println!("Creating pool...");
    let pool = zero_pool::new();

    let mut result = 0u64;
    let task = DebugTask::new(1, 1000, &mut result);

    println!("Submitting task...");
    let future = pool.submit_task(&task, debug_task_fn);

    println!("Waiting with 5 second timeout...");
    let start = Instant::now();
    let completed = future.wait_timeout(Duration::from_secs(5));
    let duration = start.elapsed();

    if completed {
        println!(
            "Task completed within timeout in {:?}, result: {}",
            duration, result
        );
    } else {
        println!("Task TIMED OUT after {:?}", duration);
        panic!("Task should have completed within 5 seconds");
    }

    println!("Dropping pool...");
    drop(pool);
    println!("Pool dropped successfully");
}

#[test]
fn debug_test_empty_batch() {
    println!("Creating pool...");
    let pool = zero_pool::new();

    println!("Creating empty batch...");
    let empty_tasks: Vec<DebugTask> = Vec::new();
    let batch = pool.submit_batch_uniform(debug_task_fn, &empty_tasks);

    println!("Checking if empty batch is immediately complete...");
    assert!(
        batch.is_complete(),
        "Empty batch should be immediately complete"
    );

    println!("Waiting on empty batch...");
    let start = Instant::now();
    batch.wait();
    let duration = start.elapsed();

    println!("Empty batch wait completed in {:?}", duration);
    assert!(
        duration < Duration::from_millis(100),
        "Empty batch wait should be nearly instantaneous"
    );

    println!("Dropping pool...");
    drop(pool);
    println!("Pool dropped successfully");
}

#[test]
fn debug_test_worker_count_behavior() {
    for worker_count in [1, 2, 4] {
        println!("--- Testing with {} workers ---", worker_count);

        println!("Creating pool with {} workers...", worker_count);
        let pool = zero_pool::with_workers(worker_count);
        assert_eq!(pool.worker_count(), worker_count);

        let task_count = worker_count * 2;
        let mut results = vec![0u64; task_count];

        println!(
            "Creating {} tasks for {} workers...",
            task_count, worker_count
        );
        let tasks: Vec<_> = results
            .iter_mut()
            .enumerate()
            .map(|(i, result)| DebugTask::new(i, 1000, result))
            .collect();

        println!("Submitting batch...");
        let batch = pool.submit_batch_uniform(debug_task_fn, &tasks);

        println!("Waiting for completion...");
        let start = Instant::now();
        batch.wait();
        let duration = start.elapsed();

        println!("Completed in {:?}", duration);

        let completed_count = results.iter().filter(|&&r| r != 0).count();
        println!("Completed tasks: {} / {}", completed_count, task_count);
        assert_eq!(completed_count, task_count);

        println!("Dropping pool...");
        drop(pool);
        println!("Pool with {} workers dropped successfully", worker_count);
    }
}

#[test]
fn debug_test_benchmark_simulation() {
    // simulates what criterion does - multiple iterations with the same pool

    println!("Creating pool...");
    let pool = zero_pool::new();

    for iteration in 0..10 {
        println!("--- Simulation iteration {} ---", iteration);

        let task_count = 100;
        let mut results = vec![0u64; task_count];

        let tasks: Vec<_> = results
            .iter_mut()
            .enumerate()
            .map(|(i, result)| DebugTask::new(i, 100, result))
            .collect();

        println!("Submitting batch of {} tasks...", task_count);
        let batch = pool.submit_batch_uniform(debug_task_fn, &tasks);

        println!("Waiting for batch...");
        let start = Instant::now();
        let completed = batch.wait_timeout(Duration::from_secs(5));
        let duration = start.elapsed();

        if !completed {
            println!(
                "HANG DETECTED: Iteration {} timed out after {:?}",
                iteration, duration
            );
            println!("Pool pending: {}", pool.total_pending());
            panic!(
                "Benchmark simulation hang detected at iteration {}",
                iteration
            );
        }

        println!("Iteration {} completed in {:?}", iteration, duration);

        let completed_count = results.iter().filter(|&&r| r != 0).count();
        if completed_count != task_count {
            println!(
                "ERROR: Only {} / {} tasks completed",
                completed_count, task_count
            );
            panic!("Not all tasks completed in iteration {}", iteration);
        }

        println!("Pool pending after iteration: {}", pool.total_pending());
    }

    println!("All simulation iterations completed successfully");
    println!("Dropping pool...");
    drop(pool);
    println!("Pool dropped successfully");
}

#[test]
fn debug_test_stress_rapid_batches() {
    println!("Creating pool...");
    let pool = zero_pool::new();

    for batch_num in 0..5 {
        println!("--- Stress batch {} ---", batch_num);

        let mut results = vec![0u64; 50];
        let tasks: Vec<_> = results
            .iter_mut()
            .enumerate()
            .map(|(i, result)| DebugTask::new(i, 200, result))
            .collect();

        println!("Submitting stress batch {}...", batch_num);
        let batch = pool.submit_batch_uniform(debug_task_fn, &tasks);

        println!("Waiting for stress batch {}...", batch_num);
        let start = Instant::now();
        let completed = batch.wait_timeout(Duration::from_secs(10));
        let duration = start.elapsed();

        if !completed {
            println!("STRESS HANG: Batch {} timed out", batch_num);
            panic!("Stress test hang at batch {}", batch_num);
        }

        println!("Stress batch {} completed in {:?}", batch_num, duration);

        // No delay between batches - stress the system
    }

    println!("Stress test completed successfully");
    drop(pool);
    println!("Pool dropped successfully");
}

#[test]
fn debug_benchmark_simple_small_exact() {
    println!("Creating pool (as in benchmark)...");
    let pool = zero_pool::new();

    // Simulate multiple benchmark iterations like Criterion does
    for iteration in 0..50 {
        println!("Benchmark iteration {}", iteration);

        let mut results = vec![0u64; 100];
        let tasks: Vec<_> = results
            .iter_mut()
            .map(|result| SimpleTask::new(100, result))
            .collect();

        let start = Instant::now();
        let batch = pool.submit_batch_uniform(simple_task_fn, &tasks);

        println!("  Waiting for batch {} completion...", iteration);
        let wait_start = Instant::now();
        let completed = batch.wait_timeout(Duration::from_secs(30));
        let wait_duration = wait_start.elapsed();

        if !completed {
            println!(
                "HANG DETECTED at iteration {} after {:?}",
                iteration, wait_duration
            );
            println!("Pool pending: {}", pool.total_pending());
            panic!("Benchmark simulation hang at iteration {}", iteration);
        }

        black_box(results);
        println!(
            "  Iteration {} completed in {:?}",
            iteration,
            start.elapsed()
        );
    }

    println!("All 50 iterations completed successfully");
    drop(pool);
    println!("Pool dropped");
}

#[test]
fn debug_benchmark_simple_large_exact() {
    println!("Creating pool (as in benchmark)...");
    let pool = zero_pool::new();

    // Fewer iterations for the large test since it's more expensive
    for iteration in 0..10 {
        println!("Large benchmark iteration {}", iteration);

        let mut results = vec![0u64; 10_000];
        let tasks: Vec<_> = results
            .iter_mut()
            .map(|result| SimpleTask::new(100, result))
            .collect();

        let start = Instant::now();
        let batch = pool.submit_batch_uniform(simple_task_fn, &tasks);

        println!("  Waiting for large batch {} completion...", iteration);
        let wait_start = Instant::now();
        let completed = batch.wait_timeout(Duration::from_secs(60));
        let wait_duration = wait_start.elapsed();

        if !completed {
            println!(
                "LARGE HANG DETECTED at iteration {} after {:?}",
                iteration, wait_duration
            );
            println!("Pool pending: {}", pool.total_pending());
            panic!("Large benchmark simulation hang at iteration {}", iteration);
        }

        black_box(results);
        println!(
            "  Large iteration {} completed in {:?}",
            iteration,
            start.elapsed()
        );
    }

    println!("All 10 large iterations completed successfully");
    drop(pool);
    println!("Pool dropped");
}

#[test]
fn debug_benchmark_complex_exact() {
    println!("Creating pool (as in benchmark)...");
    let pool = zero_pool::new();

    for iteration in 0..20 {
        println!("Complex benchmark iteration {}", iteration);

        let mut results = vec![0.0; 100];
        let tasks: Vec<_> = results
            .iter_mut()
            .map(|result| ComplexTask::new(50, result))
            .collect();

        let start = Instant::now();
        let batch = pool.submit_batch_uniform(complex_task_fn, &tasks);

        println!("  Waiting for complex batch {} completion...", iteration);
        let wait_start = Instant::now();
        let completed = batch.wait_timeout(Duration::from_secs(60));
        let wait_duration = wait_start.elapsed();

        if !completed {
            println!(
                "COMPLEX HANG DETECTED at iteration {} after {:?}",
                iteration, wait_duration
            );
            println!("Pool pending: {}", pool.total_pending());
            panic!(
                "Complex benchmark simulation hang at iteration {}",
                iteration
            );
        }

        black_box(results);
        println!(
            "  Complex iteration {} completed in {:?}",
            iteration,
            start.elapsed()
        );
    }

    println!("All 20 complex iterations completed successfully");
    drop(pool);
    println!("Pool dropped");
}

#[test]
fn debug_criterion_style_timing() {
    println!("Creating pool...");
    let pool = zero_pool::new();

    // Simulate Criterion's warmup + measurement phases
    println!("=== WARMUP PHASE ===");
    for iteration in 0..10 {
        println!("Warmup iteration {}", iteration);

        let mut results = vec![0u64; 100];
        let tasks: Vec<_> = results
            .iter_mut()
            .map(|result| SimpleTask::new(100, result))
            .collect();

        let batch = pool.submit_batch_uniform(simple_task_fn, &tasks);
        let completed = batch.wait_timeout(Duration::from_secs(10));

        if !completed {
            panic!("Warmup hang at iteration {}", iteration);
        }

        black_box(results);
    }

    println!("=== MEASUREMENT PHASE ===");
    let mut timings = Vec::new();
    for iteration in 0..100 {
        if iteration % 10 == 0 {
            println!("Measurement iteration {}", iteration);
        }

        let mut results = vec![0u64; 100];
        let tasks: Vec<_> = results
            .iter_mut()
            .map(|result| SimpleTask::new(100, result))
            .collect();

        let start = Instant::now();
        let batch = pool.submit_batch_uniform(simple_task_fn, &tasks);
        let completed = batch.wait_timeout(Duration::from_secs(10));
        let duration = start.elapsed();

        if !completed {
            panic!("Measurement hang at iteration {}", iteration);
        }

        timings.push(duration);
        black_box(results);
    }

    let avg_time = timings.iter().sum::<Duration>() / timings.len() as u32;
    println!("Average timing: {:?}", avg_time);
    println!("All Criterion-style iterations completed successfully");

    drop(pool);
    println!("Pool dropped");
}

#[test]
fn debug_memory_pressure_test() {
    println!("Creating pool...");
    let pool = zero_pool::new();

    // Create lots of data to simulate memory pressure during benchmarks
    let mut all_results = Vec::new();

    for iteration in 0..20 {
        println!("Memory pressure iteration {}", iteration);

        let mut results = vec![0u64; 1000];
        let tasks: Vec<_> = results
            .iter_mut()
            .map(|result| SimpleTask::new(500, result))
            .collect();

        let batch = pool.submit_batch_uniform(simple_task_fn, &tasks);
        let completed = batch.wait_timeout(Duration::from_secs(15));

        if !completed {
            println!("Memory pressure hang at iteration {}", iteration);
            panic!("Memory pressure test hang");
        }

        // Keep all results in memory to create pressure
        all_results.push(results);
        black_box(&all_results);

        println!(
            "  Iteration {} completed, total memory: {} vectors",
            iteration,
            all_results.len()
        );
    }

    println!("Memory pressure test completed successfully");
    drop(pool);
    println!("Pool dropped");
}

#[test]
fn debug_exact_benchmark_reproduction() {
    // This exactly mirrors your benchmark structure
    println!("Creating pool outside iteration loop...");
    let pool = zero_pool::new();

    let iteration_count = 100;
    println!(
        "Running {} iterations with same pool (exactly like benchmark)...",
        iteration_count
    );

    for i in 0..iteration_count {
        if i % 10 == 0 {
            println!("Iteration {} / {}", i, iteration_count);
        }

        // Exact code from your benchmark
        let mut results = vec![0u64; 100];
        let tasks: Vec<_> = results
            .iter_mut()
            .map(|result| SimpleTask::new(100, result))
            .collect();

        let batch = pool.submit_batch_uniform(simple_task_fn, &tasks);

        // Use timeout to catch hangs
        let completed = batch.wait_timeout(Duration::from_secs(5));
        if !completed {
            println!("EXACT REPRODUCTION HANG at iteration {}", i);
            println!(
                "Pool state - pending: {}, workers: {}",
                pool.total_pending(),
                pool.worker_count()
            );
            panic!("Exact benchmark reproduction hang at iteration {}", i);
        }

        black_box(results);
    }

    println!("Exact benchmark reproduction completed successfully!");
    drop(pool);
    println!("Pool dropped successfully");
}

#[test]
fn test_optimized_uniform_batch() {
    let pool = zero_pool::new();
    let task_count = 1000;
    let mut results = vec![0u64; task_count];

    println!(
        "Creating {} tasks for optimized uniform batch...",
        task_count
    );
    let tasks: Vec<_> = results
        .iter_mut()
        .map(|result| SimpleTask::new(100, result))
        .collect();

    // Convert to optimized format
    let tasks_converted = zero_pool::uniform_tasks_to_pointers(simple_task_fn, &tasks);

    println!("Submitting optimized uniform batch...");
    let start = Instant::now();
    let batch = pool.submit_raw_task_batch(&tasks_converted);
    batch.wait();
    let duration = start.elapsed();

    println!("Optimized uniform batch completed in {:?}", duration);

    // Verify all tasks completed
    let completed_count = results.iter().filter(|&&r| r != 0).count();
    println!("Completed tasks: {} / {}", completed_count, task_count);
    assert_eq!(
        completed_count, task_count,
        "Not all optimized tasks completed"
    );
}

#[test]
fn test_optimized_vs_normal_uniform_equivalence() {
    let pool = zero_pool::new();
    let task_count = 500;

    // Test normal API
    let mut normal_results = vec![0u64; task_count];
    let normal_tasks: Vec<_> = normal_results
        .iter_mut()
        .map(|result| SimpleTask::new(123, result))
        .collect();

    println!("Running normal uniform batch...");
    let normal_start = Instant::now();
    let normal_batch = pool.submit_batch_uniform(simple_task_fn, &normal_tasks);
    normal_batch.wait();
    let normal_duration = normal_start.elapsed();

    // Test optimized API with same work
    let mut optimized_results = vec![0u64; task_count];
    let optimized_tasks: Vec<_> = optimized_results
        .iter_mut()
        .map(|result| SimpleTask::new(123, result))
        .collect();

    let tasks_converted = zero_pool::uniform_tasks_to_pointers(simple_task_fn, &optimized_tasks);

    println!("Running optimized uniform batch...");
    let optimized_start = Instant::now();
    let optimized_batch = pool.submit_raw_task_batch(&tasks_converted);
    optimized_batch.wait();
    let optimized_duration = optimized_start.elapsed();

    println!("Normal API duration: {:?}", normal_duration);
    println!("Optimized API duration: {:?}", optimized_duration);

    // Verify results are identical
    assert_eq!(normal_results.len(), optimized_results.len());
    for (i, (&normal, &optimized)) in normal_results
        .iter()
        .zip(optimized_results.iter())
        .enumerate()
    {
        assert_eq!(
            normal, optimized,
            "Result mismatch at index {}: normal={}, optimized={}",
            i, normal, optimized
        );
    }

    println!("Results are identical between normal and optimized APIs");
}

#[test]
fn test_optimized_mixed_batch() {
    let pool = zero_pool::new();

    // Prepare different types of tasks
    let mut simple_result = 0u64;
    let simple_task = SimpleTask::new(50000, &mut simple_result);

    let mut prime_result = false;
    let prime_task = PrimeTask::new(982451653, &mut prime_result);

    let mut matrix_result = 0.0f64;
    let matrix_task = MatrixTask::new(50, &mut matrix_result);

    let mut sort_data = Vec::new();
    let sort_task = SortTask::new(1000, &mut sort_data);

    println!("Converting mixed tasks to optimized format...");
    let mixed_tasks = vec![
        (
            simple_cpu_task as TaskFnPointer,
            &simple_task as *const _ as TaskParamPointer,
        ),
        (
            is_prime_task as TaskFnPointer,
            &prime_task as *const _ as TaskParamPointer,
        ),
        (
            matrix_multiply_task as TaskFnPointer,
            &matrix_task as *const _ as TaskParamPointer,
        ),
        (
            sort_data_task as TaskFnPointer,
            &sort_task as *const _ as TaskParamPointer,
        ),
    ];

    println!("Submitting optimized mixed batch...");
    let start = Instant::now();
    let batch = pool.submit_raw_task_batch(&mixed_tasks);
    batch.wait();
    let duration = start.elapsed();

    println!("Optimized mixed batch completed in {:?}", duration);

    // Verify all tasks completed
    assert_ne!(simple_result, 0, "Simple task did not complete");
    assert_ne!(matrix_result, 0.0, "Matrix task did not complete");
    assert!(!sort_data.is_empty(), "Sort task did not complete");

    println!("Simple task result: {}", simple_result);
    println!("Prime check result: {}", prime_result);
    println!("Matrix result: {}", matrix_result);
    println!("Sort result length: {}", sort_data.len());
}

#[test]
fn test_optimized_vs_normal_mixed_equivalence() {
    let pool = zero_pool::new();

    // Test normal mixed API
    let mut normal_simple_result = 0u64;
    let normal_simple_task = SimpleTask::new(12345, &mut normal_simple_result);

    let mut normal_prime_result = false;
    let normal_prime_task = PrimeTask::new(97, &mut normal_prime_result);

    println!("Running normal mixed batch...");
    let normal_start = Instant::now();
    let normal_batch = zero_pool::zp_submit_batch_mixed!(
        pool,
        [
            (&normal_simple_task, simple_cpu_task),
            (&normal_prime_task, is_prime_task),
        ]
    );
    normal_batch.wait();
    let normal_duration = normal_start.elapsed();

    // Test optimized mixed API with same work
    let mut optimized_simple_result = 0u64;
    let optimized_simple_task = SimpleTask::new(12345, &mut optimized_simple_result);

    let mut optimized_prime_result = false;
    let optimized_prime_task = PrimeTask::new(97, &mut optimized_prime_result);

    let mixed_tasks = vec![
        (
            simple_cpu_task as TaskFnPointer,
            &optimized_simple_task as *const _ as TaskParamPointer,
        ),
        (
            is_prime_task as TaskFnPointer,
            &optimized_prime_task as *const _ as TaskParamPointer,
        ),
    ];

    println!("Running optimized mixed batch...");
    let optimized_start = Instant::now();
    let optimized_batch = pool.submit_raw_task_batch(&mixed_tasks);
    optimized_batch.wait();
    let optimized_duration = optimized_start.elapsed();

    println!("Normal mixed API duration: {:?}", normal_duration);
    println!("Optimized mixed API duration: {:?}", optimized_duration);

    // Verify results are identical
    assert_eq!(
        normal_simple_result, optimized_simple_result,
        "Simple task results differ: normal={}, optimized={}",
        normal_simple_result, optimized_simple_result
    );
    assert_eq!(
        normal_prime_result, optimized_prime_result,
        "Prime task results differ: normal={}, optimized={}",
        normal_prime_result, optimized_prime_result
    );

    println!("Results are identical between normal and optimized mixed APIs");
}

#[test]
fn test_optimized_empty_batch() {
    let pool = zero_pool::new();

    // Test empty optimized uniform batch
    let empty_tasks: Vec<SimpleTask> = Vec::new();
    let tasks_converted = zero_pool::uniform_tasks_to_pointers(simple_task_fn, &empty_tasks);

    println!("Submitting empty optimized uniform batch...");
    let batch = pool.submit_raw_task_batch(&tasks_converted);
    assert!(
        batch.is_complete(),
        "Empty optimized batch should be immediately complete"
    );
    batch.wait();

    // Test empty optimized mixed batch
    let empty_mixed_converted: Vec<WorkItem> = Vec::new();

    println!("Submitting empty optimized mixed batch...");
    let empty_mixed_batch = pool.submit_raw_task_batch(&empty_mixed_converted);
    assert!(
        empty_mixed_batch.is_complete(),
        "Empty optimized mixed batch should be immediately complete"
    );
    empty_mixed_batch.wait();

    println!("Empty optimized batch tests completed successfully");
}

#[test]
fn test_optimized_reuse_pattern() {
    let pool = zero_pool::new();
    let task_count = 100;

    // Create tasks once and reuse the converted pointers (like in benchmarks)
    let mut results = vec![0u64; task_count];
    let tasks: Vec<_> = results
        .iter_mut()
        .map(|result| SimpleTask::new(500, result))
        .collect();

    // Convert once, reuse multiple times
    let tasks_converted = zero_pool::uniform_tasks_to_pointers(simple_task_fn, &tasks);

    println!("Running {} iterations with reused optimized tasks...", 10);
    for iteration in 0..10 {
        println!("Reuse iteration {}", iteration);

        // Clear previous results
        for result in results.iter_mut() {
            *result = 0;
        }

        let start = Instant::now();
        let batch = pool.submit_raw_task_batch(&tasks_converted);
        batch.wait();
        let duration = start.elapsed();

        println!("  Iteration {} completed in {:?}", iteration, duration);

        // Verify all tasks completed
        let completed_count = results.iter().filter(|&&r| r != 0).count();
        assert_eq!(
            completed_count, task_count,
            "Not all tasks completed in reuse iteration {}",
            iteration
        );

        black_box(results.clone());
    }

    println!("Optimized task reuse pattern test completed successfully");
}

#[test]
fn test_optimized_stress_test() {
    let pool = zero_pool::new();

    for batch_num in 0..20 {
        println!("Optimized stress batch {}", batch_num);

        let mut results = vec![0u64; 200];
        let tasks: Vec<_> = results
            .iter_mut()
            .enumerate()
            .map(|(i, result)| SimpleTask::new(100 + i * 10, result))
            .collect();

        let tasks_converted = zero_pool::uniform_tasks_to_pointers(simple_task_fn, &tasks);

        let start = Instant::now();
        let batch = pool.submit_raw_task_batch(&tasks_converted);
        let completed = batch.wait_timeout(Duration::from_secs(10));
        let duration = start.elapsed();

        if !completed {
            panic!("Optimized stress test hang at batch {}", batch_num);
        }

        println!(
            "  Optimized stress batch {} completed in {:?}",
            batch_num, duration
        );

        // Verify all completed
        let completed_count = results.iter().filter(|&&r| r != 0).count();
        assert_eq!(
            completed_count, 200,
            "Not all optimized stress tasks completed in batch {}",
            batch_num
        );
    }

    println!("Optimized stress test completed successfully");
}
