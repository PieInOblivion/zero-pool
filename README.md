# Zero-Pool: Consistent High-Performance Thread Pool
*When microseconds matter and allocation is the enemy.*

This is an experimental thread pool implementation focused on exploring lock-free FIFO MPMC queue techniques. Consider this a performance playground rather than a production-ready library.

## Key Features:

- **Zero locks** - lock-free
- **Zero queue limit** - unbounded
- **Zero channels** - no std/crossbeam channel overhead
- **Zero virtual dispatch** - function pointer dispatch avoids vtable lookups
- **Zero core spinning** - event based
- **Zero result transport cost** - tasks write directly to caller-provided memory
- **Zero per worker queues** - single global queue structure = perfect workload balancing
- **Zero external dependencies** - standard library only and stable rust

Using a result-via-parameters pattern means workers place results into caller provided memory, removing thread transport overhead. The single global queue structure ensures optimal load balancing without the complexity of work-stealing or load redistribution algorithms.

Since the library uses raw pointers, you must ensure parameter structs remain valid until `TaskFuture::wait()` completes, result pointers remain valid until task completion, and that your task functions are thread-safe. The library provides type-safe methods like `submit_task` and `submit_batch_uniform` for convenient usage.

*Note: TaskFuture uses a small Mutex + Condvar to efficiently block waiting threads. Core pool operations remain lock-free.*

## Benchmarks
```rust
test bench_heavy_compute_rayon             ... bench:   4,848,477.55 ns/iter (+/- 754,053.45)
test bench_heavy_compute_zeropool          ... bench:   4,499,730.20 ns/iter (+/- 470,663.55)
test bench_indexed_computation_rayon       ... bench:      35,823.16 ns/iter (+/- 13,229.60)
test bench_indexed_computation_zeropool    ... bench:      36,956.51 ns/iter (+/- 3,995.92)
test bench_individual_tasks_rayon_empty    ... bench:      44,887.17 ns/iter (+/- 2,517.05)
test bench_individual_tasks_zeropool_empty ... bench:      57,069.83 ns/iter (+/- 15,891.82)
test bench_task_overhead_rayon             ... bench:      35,688.05 ns/iter (+/- 10,786.13)
test bench_task_overhead_zeropool          ... bench:      35,823.08 ns/iter (+/- 4,741.10)
```

## Example Usage

### Submitting a Single Task

```rust
use zero_pool::{ZeroPool, zp_define_task_fn, zp_write};

struct CalculationParams {
    iterations: usize,
    result: *mut u64,
}

zp_define_task_fn!(calculate_task, CalculationParams, |params| {
    let mut sum = 0u64;
    for i in 0..params.iterations {
        sum += i as u64;
    }
    zp_write!(params.result, sum);
});

let pool = ZeroPool::new();
let mut result = 0u64;
let task = CalculationParams { iterations: 1000, result: &mut result };

let future = pool.submit_task(calculate_task, &task);
future.wait();

println!("Result: {}", result);
```

### Submitting Uniform Batches

Submits multiple tasks of the same type to the thread pool.

```rust
use zero_pool::{ZeroPool, zp_define_task_fn, zp_write};

struct ComputeParams {
    work_amount: usize,
    result: *mut u64,
}

zp_define_task_fn!(compute_task, ComputeParams, |params| {
    let mut sum = 0u64;
    for i in 0..params.work_amount {
        sum += i as u64;
    }
    zp_write!(params.result, sum);
});

let pool = ZeroPool::new();
let mut results = vec![0u64; 100];

let tasks: Vec<_> = results.iter_mut().enumerate().map(|(i, result)| {
    ComputeParams { work_amount: 1000 + i * 10, result }
}).collect();

let future = pool.submit_batch_uniform(compute_task, &tasks);
future.wait();

println!("First result: {}", results[0]);
```

### Submitting Multiple Independent Tasks

You can submit individual tasks and uniform batches in parallel:

```rust
use zero_pool::{ZeroPool, zp_define_task_fn, zp_write};

// Define first task type
struct ComputeParams {
    work_amount: usize,
    result: *mut u64,
}

zp_define_task_fn!(compute_task, ComputeParams, |params| {
    let mut sum = 0u64;
    for i in 0..params.work_amount {
        sum += i as u64;
    }
    zp_write!(params.result, sum);
});

// Define second task type
struct MultiplyParams { x: u64, y: u64, result: *mut u64 }

zp_define_task_fn!(multiply_task, MultiplyParams, |params| {
    zp_write!(params.result, params.x * params.y);
});

let pool = ZeroPool::new();

// Individual task - separate memory location
let mut single_result = 0u64;
let single_task_params = ComputeParams { work_amount: 1000, result: &mut single_result };

// Uniform batch - separate memory from above
let mut batch_results = vec![0u64; 50];
let batch_task_params: Vec<_> = batch_results.iter_mut().enumerate()
    .map(|(i, result)| ComputeParams { work_amount: 500 + i, result })
    .collect();

// Submit all batches
let future1 = pool.submit_task(compute_task, &single_task_params);
let future2 = pool.submit_batch_uniform(compute_task, &batch_task_params);

// Wait on them in any order; completion order is not guaranteed
future1.wait();
future2.wait(); 

println!("Single: {}", single_result);
println!("Batch completed: {} tasks", batch_results.len());
```

### `zp_define_task_fn!`

Defines a task function that safely dereferences the parameter struct.

```rust
use zero_pool::{zp_define_task_fn, zp_write};

struct SumParams {
    iterations: usize,
    result: *mut u64,
}

zp_define_task_fn!(sum_task, SumParams, |params| {
    let mut sum = 0u64;
    for i in 0..params.iterations {
        sum += i as u64;
    }
    zp_write!(params.result, sum);
});
```

### `zp_write!`

Eliminates explicit unsafe blocks when writing to result pointers.

```rust
use zero_pool::{zp_define_task_fn, zp_write};

struct TaskParams {
    value: u64,
    result: *mut u64,
}

zp_define_task_fn!(my_task, TaskParams, |params| {
    let computed_value = params.value * 2;
    zp_write!(params.result, computed_value);
});
```

### `zp_write_indexed!`

Safely writes a value to a specific index in a Vec or array via raw pointer, useful for batch processing where each task writes to a different index.

```rust
use zero_pool::{ZeroPool, zp_define_task_fn, zp_write_indexed};

struct BatchParams {
    index: usize,
    work_size: usize,
    results: *mut Vec<u64>,
}

zp_define_task_fn!(batch_task, BatchParams, |params| {
    let mut sum = 0u64;
    for i in 0..params.work_size {
        sum += i as u64;
    }
    zp_write_indexed!(params.results, params.index, sum);
});

// Usage with a pre-allocated vector
let pool = ZeroPool::new();
let mut results = vec![0u64; 100];
let task_params: Vec<_> = (0..100).map(|i| {
    BatchParams { index: i, work_size: 1000, results: &mut results }
}).collect();

let future = pool.submit_batch_uniform(batch_task, &task_params);
future.wait();
```