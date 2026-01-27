use std::sync::atomic::{AtomicPtr, AtomicUsize, Ordering};

use crate::{TaskFnPointer, TaskParamPointer, padded_type::PaddedType, task_future::TaskFuture};

pub struct TaskBatch {
    next_byte_offset: PaddedType<AtomicUsize>,
    // pointer arithmetic instead of usize address math to preserve pointer provenance
    params_ptr: TaskParamPointer,
    param_stride: usize,
    params_total_bytes: usize,
    task_fn_ptr: TaskFnPointer,
    pub epoch: AtomicUsize,
    pub future: TaskFuture,
    pub next: AtomicPtr<TaskBatch>,
}

impl TaskBatch {
    pub fn new<T>(task_fn_ptr: TaskFnPointer, params: &[T], future: TaskFuture) -> Self {
        TaskBatch {
            next_byte_offset: PaddedType::new(AtomicUsize::new(0)),
            params_ptr: params.as_ptr() as TaskParamPointer,
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

    // trying to keep TaskBatch within two cache lines is crucial for performance,
    // however now having two changing atomics in line two can create cache invalidation stalls,
    // so having workers take a local version of param variables should help in most cases
    pub fn claim_next_param_local(
        &self,
        params_ptr: TaskParamPointer,
        param_stride: usize,
        params_total_bytes: usize,
    ) -> Option<TaskParamPointer> {
        let byte_offset = self
            .next_byte_offset
            .fetch_add(param_stride, Ordering::Relaxed);

        if byte_offset >= params_total_bytes {
            return None;
        }
        unsafe { Some(params_ptr.add(byte_offset)) }
    }

    pub fn has_unclaimed_tasks(&self) -> bool {
        self.next_byte_offset.load(Ordering::Relaxed) < self.params_total_bytes
    }

    pub fn get_param_run_values(&self) -> (TaskParamPointer, usize, usize) {
        (self.params_ptr, self.param_stride, self.params_total_bytes)
    }

    pub fn task_fn(&self) -> TaskFnPointer {
        self.task_fn_ptr
    }
}
