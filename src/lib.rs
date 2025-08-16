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
mod worker;

pub use pool::ZeroPool;

pub use future::WorkFuture;

// task function signature, takes raw pointer to parameters
pub type TaskFnPointer = fn(*const ());

// pointer to task parameter struct
pub type TaskParamPointer = *const ();

// tuple of the two creates one work item
pub type WorkItem = (TaskFnPointer, TaskParamPointer);

// Convert uniform tasks to pointer format
#[inline]
pub fn uniform_tasks_to_pointers<T>(task_fn: TaskFnPointer, params_vec: &[T]) -> Vec<WorkItem> {
    params_vec
        .iter()
        .map(|params| (task_fn, params as *const T as TaskParamPointer))
        .collect()
}
