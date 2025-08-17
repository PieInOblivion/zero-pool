//! # Zero-Pool: Ultra-High Performance Thread Pool
//!
//! A thread pool implementation designed for maximum performance through:
//! - Zero-overhead task submission via raw pointers  
//! - Result-via-parameters pattern (no result transport)
//! - Single global queue with optimal load balancing
//! - Function pointer dispatch (no trait objects)
//! - Lock-free queue operations with event-based worker coordination
//!
//! ## Safety
//!
//! This library achieves high performance through raw pointer usage. Users must ensure:
//! - Parameter structs remain valid until `TaskFuture::wait()` completes
//! - Result pointers remain valid until task execution finishes  
//! - Task functions are thread-safe and data-race free
//! - No undefined behavior in unsafe task code
//!
//! ## Example
//!
//! ```rust
//! use zero_pool::{ZeroPool, zp_task_params, zp_define_task_fn, zp_write};
//!
//! zp_task_params! {
//!     MyTask { value: u64, result: *mut u64 }
//! }
//!
//! zp_define_task_fn!(my_task, MyTask, |params| {
//!     zp_write!(params.result, params.value * 2);
//! });
//!
//! let pool = ZeroPool::new();
//! let mut result = 0u64;
//! let task = MyTask::new(42, &mut result);
//! pool.submit_task(my_task, &task).wait();
//! assert_eq!(result, 84);
//! ```

mod macros;
mod padded_type;
mod pool;
mod queue;
mod task_batch;
mod task_future;
mod worker;

pub use pool::ZeroPool;

pub use task_future::TaskFuture;

/// Function pointer type for task execution
///
/// Tasks receive a raw pointer to their parameter struct and must
/// cast it to the appropriate type for safe access.
pub type TaskFnPointer = fn(*const ());

/// Raw pointer to task parameter struct
///
/// This is type-erased for uniform storage but must be cast back
/// to the original parameter type within the task function.
pub type TaskParamPointer = *const ();

/// A work item containing a task function and its parameters
///
/// This tuple represents a single unit of work that can be
/// executed by a worker thread.
pub type TaskItem = (TaskFnPointer, TaskParamPointer);

/// Convert a slice of uniform task parameters to work items
///
/// This is a performance optimization that pre-converts parameter
/// pointers, avoiding repeated conversions during batch submission.
/// Useful when you need to submit the same batch multiple times.
///
/// # Examples
///
/// ```rust
/// use zero_pool::{ZeroPool, uniform_tasks_to_pointers, zp_task_params, zp_define_task_fn, zp_write};
///
/// zp_task_params! {
///     MyTask { value: u64, result: *mut u64 }
/// }
///
/// zp_define_task_fn!(my_task_fn, MyTask, |params| {
///     zp_write!(params.result, params.value * 2);
/// });
///
/// let pool = ZeroPool::new();
/// let mut results = [0u64; 2];
/// let tasks = [
///     MyTask::new(1, &mut results[0]),
///     MyTask::new(2, &mut results[1])
/// ];
///
/// // Convert once, reuse multiple times
/// let work_items = uniform_tasks_to_pointers(my_task_fn, &tasks);
///
/// for _ in 0..5 {
///     let future = pool.submit_raw_task_batch(&work_items);
///     future.wait();
/// }
/// ```
#[inline]
pub fn uniform_tasks_to_pointers<T>(task_fn: TaskFnPointer, params_vec: &[T]) -> Vec<TaskItem> {
    params_vec
        .iter()
        .map(|params| (task_fn, params as *const T as TaskParamPointer))
        .collect()
}
