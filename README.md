# Zero-Pool: Consistent High-Performance Thread Pool
*When microseconds matter and allocation is the enemy.*

This is an experimental thread pool implementation focused on exploring lock-free MPMC queue techniques and zero-allocation task dispatch. Consider this a performance playground rather than a production-ready library.

## Key Features:

- **16 bytes per task** - minimal memory footprint per work item
- **Zero locks** - lock free queue
- **Zero queue limit** - unbounded
- **Zero virtual dispatch** - function pointer dispatch avoids vtable lookups
- **Zero core spinning** - event based
- **Zero result transport cost** - tasks write directly to caller-provided memory
- **Zero per worker queues** - single global queue structure
- **Zero external dependencies** - standard library and stable rust only

Workers are only passed 16 bytes per work item, a function pointer and a struct pointer. Using a result-via-parameters pattern means workers place results into caller provided memory, removing thread transport overhead. The single global queue structure ensures optimal load balancing without the complexity of work-stealing or load redistribution algorithms.

Since the library uses raw pointers, you must ensure parameter structs remain valid until `TaskFuture::wait()` completes, result pointers remain valid until task completion, and that your task functions are thread-safe. The library provides type-safe methods like `submit_task` and `submit_batch_uniform` for convenient usage.

## Benchmarks
```rust
test bench_heavy_compute_rayon                    ... bench:   4,844,119.25 ns/iter (+/- 626,564.62)
test bench_heavy_compute_rayon_optimised          ... bench:   4,935,556.95 ns/iter (+/- 454,298.12)
test bench_heavy_compute_zeropool                 ... bench:   4,390,880.40 ns/iter (+/- 347,767.12)
test bench_heavy_compute_zeropool_optimised       ... bench:   4,407,382.45 ns/iter (+/- 336,057.06)
test bench_indexed_computation_rayon              ... bench:      39,135.11 ns/iter (+/- 14,160.70)
test bench_indexed_computation_rayon_optimised    ... bench:      34,639.97 ns/iter (+/- 7,624.86)
test bench_indexed_computation_zeropool           ... bench:      50,064.12 ns/iter (+/- 4,719.97)
test bench_indexed_computation_zeropool_optimised ... bench:      40,170.21 ns/iter (+/- 5,019.51)
test bench_task_overhead_rayon                    ... bench:      39,940.40 ns/iter (+/- 9,373.38)
test bench_task_overhead_rayon_optimised          ... bench:      40,994.87 ns/iter (+/- 13,775.16)
test bench_task_overhead_zeropool                 ... bench:      50,517.70 ns/iter (+/- 3,595.43)
test bench_task_overhead_zeropool_optimised       ... bench:      45,036.93 ns/iter (+/- 7,731.93)
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

You can submit individual tasks, uniform batches, and mixed batches in parallel:

```rust
use zero_pool::{ZeroPool, zp_submit_batch_mixed, zp_define_task_fn, zp_write};

// Define task types (assuming compute_task is already defined)
struct MultiplyParams { x: u64, y: u64, result: *mut u64 }

zp_define_task_fn!(multiply_task, MultiplyParams, |params| {
    zp_write!(params.result, params.x * params.y);
});

let pool = ZeroPool::new();

// Individual tasks - separate memory locations
let mut single_result = 0u64;
let single_task = ComputeParams { work_amount: 1000, result: &mut single_result };

// Uniform batch - separate memory from above
let mut batch_results = vec![0u64; 50];
let batch_tasks: Vec<_> = batch_results.iter_mut().enumerate()
    .map(|(i, result)| ComputeParams { work_amount: 500 + i, result })
    .collect();

// Mixed batch - separate memory from above  
let mut add_result = 0u64;
let mut multiply_result = 0u64;
let compute_mixed_params = ComputeParams { work_amount: 2000, result: &mut add_result };
let multiply_mixed_params = MultiplyParams { x: 6, y: 7, result: &mut multiply_result };

// Batches are queued in order but tasks run concurrently
let future1 = pool.submit_task(compute_task, &single_task);
let future2 = pool.submit_batch_uniform(compute_task, &batch_tasks);
let future3 = zp_submit_batch_mixed!(pool, [
    (&compute_mixed_params, compute_task),
    (&multiply_mixed_params, multiply_task),
]);

// Wait on them in any order; completion order is not guaranteed
future1.wait();
future2.wait(); 
future3.wait();

println!("Single: {}", single_result);
println!("Batch completed: {} tasks", batch_results.len());
println!("Mixed: {} and {}", add_result, multiply_result);
```

### Performance Optimisation: Pre-converted Tasks

For hot paths where you submit the same tasks repeatedly, you can pre-convert tasks to avoid repeated pointer conversions:

```rust
let pool = ZeroPool::new();
let mut results = vec![0u64; 100];

let tasks: Vec<_> = results.iter_mut().map(|result| {
    ComputeParams { work_amount: 1000, result }
}).collect();

// Convert once, reuse multiple times
let tasks_converted = uniform_tasks_to_pointers(compute_task, &tasks);

// Submit multiple batches with zero conversion overhead
let futures: Vec<_> = (0..3).map(|_| {
    pool.submit_raw_task_batch(&tasks_converted)
}).collect();

// Submit multiple batches with zero conversion overhead
for _ in 0..3 {
    let future = pool.submit_raw_task_batch(&tasks_converted);
    future.wait();
}
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

Optional macro that eliminates explicit unsafe blocks when writing to params struct.

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
let tasks: Vec<_> = (0..100).map(|i| {
    BatchParams { index: i, work_size: 1000, results: &mut results }
}).collect();

let future = pool.submit_batch_uniform(batch_task, &tasks);
future.wait();
```

### `zp_submit_batch_mixed!`

Submits multiple tasks of different types to the thread pool.

```rust
use zero_pool::{ZeroPool, zp_submit_batch_mixed, zp_define_task_fn, zp_write};

// First task type
struct AddParams {
    a: u64,
    b: u64,
    result: *mut u64,
}

zp_define_task_fn!(add_task, AddParams, |params| {
    zp_write!(params.result, params.a + params.b);
});

// Second task type
struct MultiplyParams {
    x: u64,
    y: u64,
    result: *mut u64,
}

zp_define_task_fn!(multiply_task, MultiplyParams, |params| {
    zp_write!(params.result, params.x * params.y);
});

let pool = ZeroPool::new();
let mut add_result = 0u64;
let mut multiply_result = 0u64;

let add = AddParams { a: 5, b: 3, result: &mut add_result };
let multiply = MultiplyParams { x: 4, y: 7, result: &mut multiply_result };

let future = zp_submit_batch_mixed!(pool, [
    (&add, add_task),
    (&multiply, multiply_task),
]);
future.wait();

println!("5 + 3 = {}", add_result);
println!("4 * 7 = {}", multiply_result);
```