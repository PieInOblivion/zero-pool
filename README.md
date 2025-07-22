# Zero-Pool: Zero-Allocation Thread Pool
*When microseconds matter and allocation is the enemy.*

A thread pool designed for maximum performance through zero allocation overhead and minimal runtime costs. Zero-Pool achieves extreme efficiency by eliminating per-task allocations, using raw pointers for parameters, and employing per-worker queues with efficient coordination.

## Key Features

- **24 bytes per task** - minimal memory footprint per work item
- **Zero allocation overhead per task** - raw pointer parameters eliminate boxing overhead
- **Contention-free work queues** - per-worker queues eliminate shared queue bottlenecks
- **Zero virtual dispatch** - function pointer dispatch avoids vtable lookups entirely
- **Zero result transport cost** - tasks write directly to caller-provided memory
- **Minimal coordination overhead** - efficient condvar blocking without polling
- **Standard library only** - no external dependencies, stable Rust compatible

Zero-Pool achieves its performance through an architecture where each worker thread operates on its own dedicated queue, eliminating contention and cache line bouncing. Tasks are distributed round-robin across workers and execute using function pointers rather than trait objects, avoiding vtable lookups entirely. The result-via-parameters pattern means tasks write results directly to caller-provided memory, eliminating any transport overhead. Workers use condvar coordination to block efficiently when no work is available, avoiding both polling overhead and unnecessary context switching.

Since the library uses raw pointers for maximum performance, you must ensure parameter structs remain valid until `future.wait()` completes, result pointers remain valid until task completion, and that your task functions are thread-safe. The library provides type-safe macros like `zp_submit_task!` and `zp_submit_batch_uniform!` for convenient usage while maintaining zero allocation overhead.

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

### `zp_write_result!`

Optional macro that eliminates explicit unsafe blocks when writing task results.

```rust
use zero_pool::{zp_define_task_fn, zp_write_result};

zp_define_task_fn!(my_task, MyTask, |params| {
    let mut sum = 0u64;
    for i in 0..params.iterations {
        sum += i as u64;
    }
    zp_write_result!(params.result, sum);
});
```

### `zp_submit_task!`

Submits a single task to the thread pool.

```rust
use zero_pool::zp_submit_task;

let pool = zero_pool::new();
let mut result = 0u64;
let task = MyTask::new(1000, &mut result);

let future = zp_submit_task!(pool, task, my_task);
future.wait();
```

### `zp_submit_batch_uniform!`

Submits multiple tasks of the same type to the thread pool.

```rust
use zero_pool::zp_submit_batch_uniform;

let pool = zero_pool::new();
let mut results = vec![0u64; 100];

let tasks: Vec<_> = results.iter_mut().map(|result| {
    MyTask::new(1000, result)
}).collect();

let batch = zp_submit_batch_uniform!(pool, tasks, my_task);
batch.wait();
```

### `zp_submit_batch_mixed!`

Submits multiple tasks of different types to the thread pool.

```rust
use zero_pool::zp_submit_batch_mixed;

// Assume we have another task type called OtherTask with other_task function
let pool = zero_pool::new();
let mut result1 = 0u64;
let mut result2 = 0u64;

let task1 = MyTask::new(1000, &mut result1);
let task2 = OtherTask::new(42, &mut result2);

let batch = zp_submit_batch_mixed!(pool, [
    (task1, my_task),
    (task2, other_task),
]);
batch.wait();
```