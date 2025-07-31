# Zero-Pool: Zero-Allocation Thread Pool
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

Workers are only passed 16 bytes per work item, a function pointer and a struct pointer. Using a result-via-parameters pattern means workers place results into caller provided memory, removing thread transport overhead.

Since the library uses raw pointers, you must ensure parameter structs remain valid until `future.wait()` completes, result pointers remain valid until task completion, and that your task functions are thread-safe. The library provides type-safe macros like `zp_submit_task!` and `zp_submit_batch_uniform!` for convenient usage.

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

let batch = zp_submit_batch_uniform!(pool, tasks, batch_task);
batch.wait();
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