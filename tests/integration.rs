use std::{
    hint::black_box,
    time::{Duration, Instant},
};
use zero_pool::{
    self, zp_define_task_fn, zp_submit_batch_mixed, zp_submit_batch_uniform, zp_submit_task,
    zp_task_params, zp_write,
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
    let future = zp_submit_task!(pool, task, simple_cpu_task);
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

    let batch = zp_submit_batch_uniform!(pool, tasks, simple_cpu_task);
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
    let empty_batch = zp_submit_batch_uniform!(pool, empty_tasks, simple_cpu_task);

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

    let batch = zp_submit_batch_uniform!(pool, tasks, simple_cpu_task);

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

        let batch = zp_submit_batch_uniform!(pool, tasks, simple_cpu_task);

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
        let future = zp_submit_task!(pool2, task, simple_cpu_task);
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

    let _batch = zp_submit_batch_uniform!(pool, tasks, simple_cpu_task);

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
    println!("\n=== DEBUG: Single Task Test ===");

    println!("Creating pool...");
    let pool = zero_pool::new();
    println!("Pool created with {} workers", pool.worker_count());

    let mut result = 0u64;
    let task = DebugTask::new(1, 1000, &mut result);

    println!("Submitting single task...");
    let future = zp_submit_task!(pool, task, debug_task_fn);

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
    println!("\n=== DEBUG: Small Batch Test ===");

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
    let batch = zp_submit_batch_uniform!(pool, tasks, debug_task_fn);
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
    println!("\n=== DEBUG: Rapid Pool Creation/Destruction ===");

    for iteration in 0..3 {
        println!("--- Iteration {} ---", iteration);

        println!("Creating pool...");
        let pool = zero_pool::new();

        let mut result = 0u64;
        let task = DebugTask::new(iteration, 500, &mut result);

        println!("Submitting task...");
        let future = zp_submit_task!(pool, task, debug_task_fn);

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
    println!("\n=== DEBUG: Timeout Behavior Test ===");

    println!("Creating pool...");
    let pool = zero_pool::new();

    let mut result = 0u64;
    let task = DebugTask::new(1, 1000, &mut result);

    println!("Submitting task...");
    let future = zp_submit_task!(pool, task, debug_task_fn);

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
    println!("\n=== DEBUG: Empty Batch Test ===");

    println!("Creating pool...");
    let pool = zero_pool::new();

    println!("Creating empty batch...");
    let empty_tasks: Vec<DebugTask> = Vec::new();
    let batch = zp_submit_batch_uniform!(pool, empty_tasks, debug_task_fn);

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
    println!("\n=== DEBUG: Different Worker Count Test ===");

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
        let batch = zp_submit_batch_uniform!(pool, tasks, debug_task_fn);

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
    println!("\n=== DEBUG: Benchmark Simulation Test ===");
    println!("This simulates what criterion does - multiple iterations with the same pool");

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
        let batch = zp_submit_batch_uniform!(pool, tasks, debug_task_fn);

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
    println!("\n=== DEBUG: Stress Test - Rapid Batches ===");

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
        let batch = zp_submit_batch_uniform!(pool, tasks, debug_task_fn);

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
    println!("\n=== DEBUG: Exact Simple Small Benchmark ===");

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
        let batch = zp_submit_batch_uniform!(pool, tasks, simple_task_fn);

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
    println!("\n=== DEBUG: Exact Simple Large Benchmark ===");

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
        let batch = zp_submit_batch_uniform!(pool, tasks, simple_task_fn);

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
    println!("\n=== DEBUG: Exact Complex Benchmark ===");

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
        let batch = zp_submit_batch_uniform!(pool, tasks, complex_task_fn);

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
    println!("\n=== DEBUG: Criterion-style Timing Test ===");

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

        let batch = zp_submit_batch_uniform!(pool, tasks, simple_task_fn);
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
        let batch = zp_submit_batch_uniform!(pool, tasks, simple_task_fn);
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
    println!("\n=== DEBUG: Memory Pressure Test ===");

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

        let batch = zp_submit_batch_uniform!(pool, tasks, simple_task_fn);
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
    println!("\n=== DEBUG: Exact Benchmark Code Reproduction ===");

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

        let batch = zp_submit_batch_uniform!(pool, tasks, simple_task_fn);

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
