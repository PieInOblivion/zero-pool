use std::sync::atomic::{AtomicPtr, AtomicUsize, Ordering};

use crate::{TaskFnPointer, TaskParamPointer, padded_type::PaddedType, task_future::TaskFuture};

pub struct TaskBatch {
    next_byte_offset: PaddedType<AtomicUsize>,
    // pointer arithmetic instead of usize address math to preserve pointer provenance
    pub next: PaddedType<AtomicPtr<TaskBatch>>,
    params_ptr: TaskParamPointer,
    param_stride: usize,
    params_total_bytes: usize,
    pub fn_ptr: TaskFnPointer,
    pub future: TaskFuture,
}

impl TaskBatch {
    pub fn new(
        fn_ptr: TaskFnPointer,
        params_ptr: TaskParamPointer,
        param_stride: usize,
        params_total_bytes: usize,
        future: TaskFuture,
    ) -> *mut Self {
        Box::into_raw(Box::new(TaskBatch {
            next_byte_offset: PaddedType::new(AtomicUsize::new(0)),
            next: PaddedType::new(AtomicPtr::new(std::ptr::null_mut())),
            params_ptr,
            param_stride,
            params_total_bytes,
            fn_ptr,
            future,
        }))
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
}
