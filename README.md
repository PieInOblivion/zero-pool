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

Since the library uses raw pointers, you must ensure parameter structs remain valid until `future.wait()` completes, result pointers remain valid until task completion, and that your task functions are thread-safe. The library provides type-safe macros like `zp_submit_task!` and `zp_submit_batch_uniform!` for convenient usage.

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

**Recommended:** Use the type-erasing macros for safe and convenient task submission:

### `zp_task_params!`

Creates a task parameter struct with an automatic constructor.

```rust
use zero_pool::zp_task_params;

zp_task_params! {
    MyTask {
        iterations: usize,
        result: *mut u64,
    }
}

// Usage: MyTask::new(1000, &mut result)
```

### `zp_define_task_fn!`

Defines a task function that safely dereferences the parameter struct.

```rust
use zero_pool::zp_define_task_fn;

zp_define_task_fn!(my_task, MyTask, |params| {
    let mut sum = 0u64;
    for i in 0..params.iterations {
        sum += i as u64;
    }
    unsafe { *params.result = sum; }
});
```

### `zp_write!`

Optional macro that eliminates explicit unsafe blocks when writing to params struct.

```rust
use zero_pool::{zp_define_task_fn, zp_write};

zp_define_task_fn!(my_task, MyTask, |params| {
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
use zero_pool::{zp_task_params, zp_define_task_fn, zp_write_indexed};

zp_task_params! {
    BatchTask {
        index: usize,
        work_size: usize,
        results: *mut Vec<u64>,
    }
}

zp_define_task_fn!(batch_task, BatchTask, |params| {
    let mut sum = 0u64;
    for i in 0..params.work_size {
        sum += i as u64;
    }
    zp_write_indexed!(params.results, params.index, sum);
});

// Usage with a pre-allocated vector
let pool = zero_pool::new();
let mut results = vec![0u64; 100];
let tasks: Vec<_> = (0..100).map(|i| {
    BatchTask::new(i, 1000, &mut results)
}).collect();

let batch = pool.submit_batch_uniform(batch_task, &tasks);
batch.wait();
```

### Submitting a Single Task

```rust
use zero_pool::{zp_task_params, zp_define_task_fn, zp_write};

zp_task_params! {
    MyTask {
        iterations: usize,
        result: *mut u64,
    }
}

zp_define_task_fn!(my_task, MyTask, |params| {
    let mut sum = 0u64;
    for i in 0..params.iterations {
        sum += i as u64;
    }
    zp_write!(params.result, sum);
});

let pool = zero_pool::new();
let mut result = 0u64;
let task = MyTask::new(1000, &mut result);

let future = pool.submit_task(&task, my_task);
future.wait();

println!("Result: {}", result);
```

### Submitting Uniform Batches

Submits multiple tasks of the same type to the thread pool.

```rust
use zero_pool::{zp_task_params, zp_define_task_fn, zp_write};

zp_task_params! {
    ComputeTask {
        work_amount: usize,
        result: *mut u64,
    }
}

zp_define_task_fn!(compute_task, ComputeTask, |params| {
    let mut sum = 0u64;
    for i in 0..params.work_amount {
        sum += i as u64;
    }
    zp_write!(params.result, sum);
});

let pool = zero_pool::new();
let mut results = vec![0u64; 100];

let tasks: Vec<_> = results.iter_mut().enumerate().map(|(i, result)| {
    ComputeTask::new(1000 + i * 10, result)
}).collect();

let batch = pool.submit_batch_uniform(compute_task, &tasks);
batch.wait();

println!("First result: {}", results[0]);
```

### `zp_submit_batch_mixed!`

Submits multiple tasks of different types to the thread pool.

```rust
use zero_pool::{zp_submit_batch_mixed, zp_task_params, zp_define_task_fn, zp_write};

// First task type
zp_task_params! {
    AddTask {
        a: u64,
        b: u64,
        result: *mut u64,
    }
}

zp_define_task_fn!(add_task, AddTask, |params| {
    zp_write!(params.result, params.a + params.b);
});

// Second task type
zp_task_params! {
    MultiplyTask {
        x: u64,
        y: u64,
        result: *mut u64,
    }
}

zp_define_task_fn!(multiply_task, MultiplyTask, |params| {
    zp_write!(params.result, params.x * params.y);
});

let pool = zero_pool::new();
let mut add_result = 0u64;
let mut multiply_result = 0u64;

let add = AddTask::new(5, 3, &mut add_result);
let multiply = MultiplyTask::new(4, 7, &mut multiply_result);

let batch = zp_submit_batch_mixed!(pool, [
    (&add, add_task),
    (&multiply, multiply_task),
]);
batch.wait();

println!("5 + 3 = {}", add_result);
println!("4 * 7 = {}", multiply_result);
```

### Performance Optimization: Pre-converted Tasks

For hot paths where you submit the same tasks repeatedly, you can pre-convert tasks to avoid repeated pointer conversions:

```rust
use zero_pool::{zp_task_params, zp_define_task_fn, zp_write};

zp_task_params! {
    HotTask {
        work: usize,
        result: *mut u64,
    }
}

zp_define_task_fn!(hot_task_fn, HotTask, |params| {
    let mut sum = 0u64;
    for i in 0..params.work {
        sum = sum.wrapping_add(i as u64);
    }
    zp_write!(params.result, sum);
});

let pool = zero_pool::new();
let mut results = vec![0u64; 100];

// Create tasks once
let tasks: Vec<_> = results.iter_mut().map(|result| {
    HotTask::new(1000, result)
}).collect();

// Convert once, reuse many times
let tasks_converted = zero_pool::uniform_tasks_to_pointers(hot_task_fn, &tasks);

// Submit multiple times with zero conversion overhead
for _ in 0..10 {
    results.fill(0); // Reset results
    let batch = pool.submit_raw_task_batch(&tasks_converted);
    batch.wait();
}
```