# Zero-Pool: Consistent High-Performance Thread Pool
*When nanoseconds matter and overhead is the enemy.*

This is an experimental thread pool implementation focused on exploring lock-free FIFO MPMC queue techniques. Consider this a performance playground rather than a production-ready library.

## Key Features:

- **Zero locks** - lock-free
- **Zero queue limit** - unbounded
- **Zero channels** - no std/crossbeam channel overhead
- **Zero virtual dispatch** - function pointer dispatch avoids vtable lookups
- **Zero core spinning** - all event-based
- **Zero result transport cost** - tasks write directly to caller-provided memory
- **Zero per worker queues** - single global queue structure = perfect workload balancing
- **Zero external dependencies** - standard library only and stable rust

Using a result-via-parameters pattern means workers place results into caller provided memory, removing thread transport overhead. The single global queue structure ensures optimal load balancing without the complexity of work-stealing or load redistribution algorithms.

Because the library uses raw pointers, you must ensure parameter structs (including any pointers they contain) remain valid until task completion, and that your task functions are thread-safe.

#### Notes
- TaskFuture uses a Mutex + Condvar to block waiting threads and allow multiple threads to wait on the same task completion. All other pool operations are lock-free.
- Zero-Pool supports both explicitly creating new thread pools (`ZeroPool::new`, `ZeroPool::with_workers`) and using the global instance (`zero_pool::global_pool`).
- Task functions take a single parameter struct (e.g. `&MyTaskParams`), and the parameter name can be any valid identifier.

## Benchmarks (AMD 5900X, Linux 6.17)
```rust
test bench_heavy_compute_rayon             ... bench:   4,879,891.95 ns/iter (+/- 686,055.45)
test bench_heavy_compute_zeropool          ... bench:   4,430,550.20 ns/iter (+/- 268,825.43)
test bench_indexed_computation_rayon       ... bench:      38,455.49 ns/iter (+/- 10,196.81)
test bench_indexed_computation_zeropool    ... bench:      38,886.68 ns/iter (+/- 4,397.20)
test bench_individual_tasks_rayon_empty    ... bench:      46,323.35 ns/iter (+/- 1,932.32)
test bench_individual_tasks_zeropool_empty ... bench:      34,338.00 ns/iter (+/- 8,625.49)
test bench_task_overhead_rayon             ... bench:      38,111.82 ns/iter (+/- 9,374.71)
test bench_task_overhead_zeropool          ... bench:      35,772.84 ns/iter (+/- 5,534.74)
```

## Example Usage

### Submitting a Single Task

```rust
use zero_pool::ZeroPool;

struct CalculationParams {
    iterations: usize,
    result: *mut u64,
}

fn calculate_task(params: &CalculationParams) {
    let mut sum = 0u64;
    for i in 0..params.iterations {
        sum += i as u64;
    }
    unsafe { *params.result = sum; }
}

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
use zero_pool::ZeroPool;

struct ComputeParams {
    work_amount: usize,
    result: *mut u64,
}

fn compute_task(params: &ComputeParams) {
    let mut sum = 0u64;
    for i in 0..params.work_amount {
        sum += i as u64;
    }
    unsafe { *params.result = sum; }
}

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
use zero_pool::ZeroPool;

// Define first task type
struct ComputeParams {
    work_amount: usize,
    result: *mut u64,
}

fn compute_task(params: &ComputeParams) {
    let mut sum = 0u64;
    for i in 0..params.work_amount {
        sum += i as u64;
    }
    unsafe { *params.result = sum; }
}

// Define second task type
struct MultiplyParams { x: u64, y: u64, result: *mut u64 }

fn multiply_task(params: &MultiplyParams) {
    unsafe { *params.result = params.x * params.y; }
}

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

### Using the Global Pool

If you prefer to share a single pool across your entire application, call the global accessor. The pool is created on first use and lives for the duration of the process.

```rust
use zero_pool::global_pool;

struct ExampleParams {
    work: usize,
    result: *mut u64,
}

fn example_task(params: &ExampleParams) {
    let mut sum = 0u64;
    for i in 0..params.work {
        sum = sum.wrapping_add(i as u64);
    }
    unsafe { *params.result = sum; }
}

let pool = global_pool();
let mut result = 0u64;
let params = ExampleParams { work: 1_000, result: &mut result };

pool.submit_task(example_task, &params).wait();
```