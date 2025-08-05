// Zero-Pool: Ultra-High Performance Thread Pool
// A thread pool designed for maximum performance through:
// - Zero-overhead task submission via raw pointers
// - Result-via-parameters pattern (no result transport)
// - Per-worker queues to minimize contention
// - Function pointer dispatch (no trait objects)
//
// Safety
// This library uses raw pointers for maximum performance. You must ensure:
// - Parameter structs live until `future.wait()` completes
// - Result pointers remain valid until task completion
// - No data races in your task functions
mod future;
mod macros;
mod padded_type;
mod pool;
mod queue;
mod work_batch;
mod work_item;
mod worker;

use std::num::NonZeroUsize;

pub use pool::ThreadPool;

// task function signature, takes raw pointer to parameters
pub type TaskFnPointer = fn(*const ());

// pointer to task parameter struct
pub type TaskParamPointer = *const ();

// convenience function to create a new thread pool
pub fn new() -> ThreadPool {
    let worker_count = std::thread::available_parallelism()
        .map(NonZeroUsize::get)
        .unwrap_or(1);
    ThreadPool::new(worker_count)
}

// create thread pool with specific worker count
pub fn with_workers(worker_count: usize) -> ThreadPool {
    ThreadPool::new(worker_count)
}

// Convert uniform tasks to pointer format
pub fn uniform_tasks_to_pointers<T>(
    params_vec: &[T],
    task_fn: TaskFnPointer,
) -> Vec<(TaskParamPointer, TaskFnPointer)> {
    params_vec
        .iter()
        .map(|params| (params as *const T as TaskParamPointer, task_fn))
        .collect()
}
