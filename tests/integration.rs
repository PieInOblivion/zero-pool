use std::time::Instant;
use zero_pool::{self, zp_task_params, zp_define_task_fn, zp_submit_task, zp_submit_batch_uniform, zp_submit_batch_mixed};

// Define task parameter structures using the safe macro
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

// Define task functions using the safe macro
zp_define_task_fn!(simple_cpu_task, SimpleTask, |params| {
    let mut sum = 0u64;
    for i in 0..params.iterations {
        sum = sum.wrapping_add(i as u64 * 17 + 23);
    }
    unsafe { *params.result = sum; }
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
    
    unsafe { *params.result = is_prime; }
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
    
    unsafe { *params.result = diagonal_sum; }
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
    
    unsafe { *params.result = hash; }
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
    
    let tasks: Vec<_> = results.iter_mut().map(|result| {
        SimpleTask::new(100, result)
    }).collect();
    
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
    
    let batch = zp_submit_batch_mixed!(pool, [
        (&simple_task, simple_cpu_task),
        (&prime_task, is_prime_task),
        (&matrix_task, matrix_multiply_task),
        (&sort_task, sort_data_task),
    ]);
    
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
    let tasks: Vec<_> = results.iter_mut().enumerate().map(|(i, result)| {
        // Different work amounts
        SimpleTask::new(10000 + i * 1000, result)
    }).collect();
    
    let batch = zp_submit_batch_uniform!(pool, tasks, simple_cpu_task);
    
    // Check queue lengths while work is potentially pending
    let queue_lengths = pool.queue_lengths();
    assert_eq!(queue_lengths.len(), 1);
    println!("Queue lengths: {:?}", queue_lengths);
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
        let tasks: Vec<_> = final_results.iter_mut().map(|result| {
            SimpleTask::new(1000, result)
        }).collect();
        
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
    assert_eq!(completed_before_drop, 100, "Not all tasks completed before shutdown");
    
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
    assert_eq!(pool.queue_lengths().len(), 4);
    assert_eq!(pool.total_pending(), 0);
    
    // Submit some work to see statistics change
    let mut results = vec![0u64; 20];
    let tasks: Vec<_> = results.iter_mut().map(|result| {
        // Longer running tasks
        SimpleTask::new(50000, result)
    }).collect();
    
    let _batch = zp_submit_batch_uniform!(pool, tasks, simple_cpu_task);
    
    // Check that work was distributed
    let queue_lengths = pool.queue_lengths();
    let total_pending = pool.total_pending();
    
    println!("Queue lengths after submission: {:?}", queue_lengths);
    println!("Total pending after submission: {}", total_pending);
    
    // Should have distributed work across queues
    let queues_with_work = queue_lengths.iter().filter(|&&len| len > 0).count();
    println!("Queues with work: {}", queues_with_work);
    
    // Wait a bit for work to start
    std::thread::sleep(std::time::Duration::from_millis(10));
    
    println!("Final queue lengths: {:?}", pool.queue_lengths());
    println!("Final total pending: {}", pool.total_pending());
}