use std::sync::atomic::{AtomicPtr, Ordering};

use crate::{
    TaskFnPointer, TaskParamPointer, padded_type::PaddedAtomicUsize, task_future::TaskFuture,
};

pub struct TaskBatch {
    next_item: PaddedAtomicUsize,
    task_fn_ptr: TaskFnPointer,
    params: Box<[TaskParamPointer]>,
    pub future: TaskFuture,
    pub next: AtomicPtr<TaskBatch>,
}

impl TaskBatch {
    pub fn new(
        task_fn_ptr: TaskFnPointer,
        params: Box<[TaskParamPointer]>,
        future: TaskFuture,
    ) -> Self {
        TaskBatch {
            next_item: PaddedAtomicUsize::new(0),
            task_fn_ptr,
            params,
            future,
            next: AtomicPtr::new(std::ptr::null_mut()),
        }
    }
    pub fn claim_next_param(&self) -> Option<TaskParamPointer> {
        let item_index = self.next_item.fetch_add(1, Ordering::Relaxed);
        self.params.get(item_index).copied()
    }

    pub fn has_unclaimed_tasks(&self) -> bool {
        self.next_item.load(Ordering::Relaxed) < self.params.len()
    }

    pub fn task_fn(&self) -> TaskFnPointer {
        self.task_fn_ptr
    }
}
