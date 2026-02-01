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
//! - Task functions take exactly one parameter (usually the task parameter struct)
//! - Task functions are thread-safe and data-race free
//! - No undefined behavior in unsafe task code
//!
//! This API is unsafe-by-contract and performs no runtime validation of these invariants.
//!
//! ## Example
//!
//! ```rust
//! use zero_pool::ZeroPool;
//!
//! struct MyTaskParams { value: u64, result: *mut u64 }
//!
//! fn my_task(params: &MyTaskParams) {
//!     unsafe { *params.result = params.value * 2; }
//! }
//!
//! let pool = ZeroPool::new();
//! let mut result = 0u64;
//! let task_params = MyTaskParams { value: 42, result: &mut result };
//! pool.submit_task(my_task, &task_params).wait();
//! assert_eq!(result, 84);
//! ```

mod padded_type;
mod pool;
mod queue;
mod startup_latch;
mod task_batch;
mod task_future;
mod worker;

use std::sync::OnceLock;

pub use pool::ZeroPool;
pub use task_future::TaskFuture;

static GLOBAL_ZP: OnceLock<ZeroPool> = OnceLock::new();

/// Returns a reference to the lazily initialized global pool.
///
/// This is the simplest way to share a single pool across your application.
/// The pool is created on first use using [`ZeroPool::new`].
#[inline]
pub fn global_pool() -> &'static ZeroPool {
    GLOBAL_ZP.get_or_init(ZeroPool::new)
}

/// Function pointer type for task execution
///
/// Tasks receive a raw pointer to their parameter struct and must
/// cast it to the appropriate type for safe access.
pub(crate) type TaskFnPointer = fn(*const u8);

/// Raw pointer to task parameter struct
///
/// This is type-erased for uniform storage but must be cast back
/// to the original parameter type within the task function.
pub(crate) type TaskParamPointer = *const u8;
