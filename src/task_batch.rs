use std::ptr::NonNull;
use std::sync::atomic::{AtomicPtr, AtomicUsize, Ordering};

use crate::{TaskFnPointer, TaskParamPointer, padded_type::PaddedType, task_future::TaskFuture};

pub struct TaskBatch {
    next_byte_offset: PaddedType<AtomicUsize>,
    // pointer arithmetic instead of usize address math to preserve pointer provenance
    params_ptr: TaskParamPointer,
    param_stride: usize,
    params_total_bytes: usize,
    pub task_fn_ptr: TaskFnPointer,
    pub epoch: AtomicUsize,
    pub future: TaskFuture,
    pub next: AtomicPtr<TaskBatch>,
}

impl TaskBatch {
    pub fn new<T>(task_fn_ptr: TaskFnPointer, params: &[T], future: TaskFuture) -> Self {
        TaskBatch {
            next_byte_offset: PaddedType::new(AtomicUsize::new(0)),
            params_ptr: NonNull::from(params).cast(),
            param_stride: std::mem::size_of::<T>(),
            params_total_bytes: std::mem::size_of_val(params),
            task_fn_ptr,
            epoch: AtomicUsize::new(0),
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
}
