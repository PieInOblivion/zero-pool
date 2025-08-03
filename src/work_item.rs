use crate::TaskFn;

#[derive(Copy, Clone)]
pub struct WorkItem {
    pub params: *const (),
    pub task_fn: TaskFn,
}

unsafe impl Send for WorkItem {}
unsafe impl Sync for WorkItem {}

impl WorkItem {
    pub fn new(params: *const (), task_fn: TaskFn) -> Self {
        WorkItem { params, task_fn }
    }
}
