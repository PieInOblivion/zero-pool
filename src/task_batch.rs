use std::sync::atomic::{AtomicPtr, AtomicUsize, Ordering};

use crate::{TaskFnPointer, TaskParamPointer, padded_type::PaddedType, task_future::TaskFuture};

pub struct TaskBatch {
    next_byte_offset: PaddedType<AtomicUsize>,
    // pointer arithmetic instead of usize address math to preserve pointer provenance
    params_ptr: TaskParamPointer,
    param_stride: usize,
    params_total_bytes: usize,
    task_fn_ptr: TaskFnPointer,
    pub future: TaskFuture,
    pub next: AtomicPtr<TaskBatch>,
}

impl TaskBatch {
    pub fn new<T>(task_fn_ptr: TaskFnPointer, params: &[T], future: TaskFuture) -> Self {
        let param_stride = std::mem::size_of::<T>();

        TaskBatch {
            next_byte_offset: PaddedType::new(AtomicUsize::new(0)),
            params_ptr: params.as_ptr() as TaskParamPointer,
            param_stride,
            params_total_bytes: param_stride * params.len(),
            task_fn_ptr,
            future,
            next: AtomicPtr::new(std::ptr::null_mut()),
        }
    }

    pub fn claim_next_param(&self) -> Option<TaskParamPointer> {
        let byte_offset = self
            .next_byte_offset
            .fetch_add(self.param_stride, Ordering::Relaxed);

        if byte_offset >= self.params_total_bytes {
            return None;
        }
        unsafe { Some(self.params_ptr.add(byte_offset)) }
    }

    pub fn has_unclaimed_tasks(&self) -> bool {
        self.next_byte_offset.load(Ordering::Relaxed) < self.params_total_bytes
    }

    pub fn task_fn(&self) -> TaskFnPointer {
        self.task_fn_ptr
    }
}
