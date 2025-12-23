use std::sync::atomic::{AtomicPtr, AtomicUsize, Ordering};

use crate::{TaskFnPointer, TaskParamPointer, padded_type::PaddedType, task_future::TaskFuture};

pub struct TaskBatch {
    next_item: PaddedType<AtomicUsize>,
    // pointer arithmetic instead of usize address math to preserve pointer provenance
    params_ptr: *const u8,
    params_len: usize,
    param_stride: usize,
    task_fn_ptr: TaskFnPointer,
    pub future: TaskFuture,
    pub next: AtomicPtr<TaskBatch>,
}

impl TaskBatch {
    pub fn new<T>(task_fn_ptr: TaskFnPointer, params: &[T], future: TaskFuture) -> Self {
        TaskBatch {
            next_item: PaddedType::new(AtomicUsize::new(0)),
            params_ptr: params.as_ptr() as *const u8,
            params_len: params.len(),
            param_stride: std::mem::size_of::<T>(),
            task_fn_ptr,
            future,
            next: AtomicPtr::new(std::ptr::null_mut()),
        }
    }

    pub fn claim_next_param(&self) -> Option<TaskParamPointer> {
        let item_index = self.next_item.fetch_add(1, Ordering::Relaxed);
        if item_index >= self.params_len {
            return None;
        }
        unsafe {
            let element_ptr = self.params_ptr.add(item_index * self.param_stride);
            Some(element_ptr as TaskParamPointer)
        }
    }

    pub fn has_unclaimed_tasks(&self) -> bool {
        self.next_item.load(Ordering::Relaxed) < self.params_len
    }

    pub fn task_fn(&self) -> TaskFnPointer {
        self.task_fn_ptr
    }
}
