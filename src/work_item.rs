use crate::{TaskFnPointer, TaskParamPointer};

#[derive(Copy, Clone)]
pub struct WorkItem {
    pub params: TaskParamPointer,
    pub task_fn: TaskFnPointer,
}

unsafe impl Send for WorkItem {}
unsafe impl Sync for WorkItem {}

impl WorkItem {
    pub fn new(params: TaskParamPointer, task_fn: TaskFnPointer) -> Self {
        WorkItem { params, task_fn }
    }
}
