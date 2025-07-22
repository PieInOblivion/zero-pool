use crate::{TaskFn, future::WorkFuture};

// Work item containing task parameters and function pointer
pub struct WorkItem {
    pub params: *const (),
    pub task_fn: TaskFn,
    pub future: WorkFuture,
}

unsafe impl Send for WorkItem {}
unsafe impl Sync for WorkItem {}

impl WorkItem {
    pub fn new(params: *const (), task_fn: TaskFn, future: WorkFuture) -> Self {
        WorkItem {
            params,
            task_fn,
            future,
        }
    }
}
