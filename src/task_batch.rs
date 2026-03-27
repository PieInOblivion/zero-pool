use std::ptr::NonNull;
use std::sync::atomic::{AtomicPtr, AtomicUsize, Ordering};
use std::thread::{self, Thread};
use std::time::{Duration, Instant};

use crate::{TaskFnPointer, TaskParamPointer, padded_type::PaddedType};

pub struct TaskBatch {
    next_byte_offset: PaddedType<AtomicUsize>,
    // pointer arithmetic instead of usize address math to preserve pointer provenance
    uncompleted: PaddedType<AtomicUsize>,
    refs: PaddedType<AtomicUsize>,
    pub next: PaddedType<AtomicPtr<TaskBatch>>,
    params_ptr: TaskParamPointer,
    param_stride: usize,
    params_total_bytes: usize,
    pub fn_ptr: TaskFnPointer,
    submitter_thread: Thread,
}

impl TaskBatch {
    pub fn new<T>(
        fn_ptr: TaskFnPointer,
        params: &[T],
        uncompleted: usize,
        refs: usize,
    ) -> *mut Self {
        Box::into_raw(Box::new(TaskBatch {
            next_byte_offset: PaddedType::new(AtomicUsize::new(0)),
            uncompleted: PaddedType::new(AtomicUsize::new(uncompleted)),
            refs: PaddedType::new(AtomicUsize::new(refs)),
            next: PaddedType::new(AtomicPtr::new(std::ptr::null_mut())),
            params_ptr: NonNull::from(params).cast(),
            param_stride: std::mem::size_of::<T>(),
            params_total_bytes: std::mem::size_of_val(params),
            fn_ptr,
            submitter_thread: thread::current(),
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

    pub fn is_complete(&self) -> bool {
        self.uncompleted.load(Ordering::Acquire) == 0
    }

    pub fn wait(&self) {
        debug_assert_eq!(
            self.submitter_thread.id(),
            thread::current().id(),
            "TaskFuture::wait() must be called from the thread that created it."
        );

        while !self.is_complete() {
            thread::park();
        }
    }

    pub fn wait_timeout(&self, timeout: Duration) -> bool {
        debug_assert_eq!(
            self.submitter_thread.id(),
            thread::current().id(),
            "TaskFuture::wait_timeout() must be called from the thread that created it."
        );

        let start = Instant::now();
        loop {
            if self.is_complete() {
                return true;
            }
            let elapsed = start.elapsed();
            if elapsed >= timeout {
                return false;
            }
            thread::park_timeout(timeout - elapsed);
        }
    }

    pub fn complete_many(&self, count: usize) {
        if self.uncompleted.fetch_sub(count, Ordering::Release) == count {
            self.submitter_thread.unpark();
        }
    }

    pub fn retain(&self) {
        self.refs.fetch_add(1, Ordering::Relaxed);
    }

    pub fn release(&self) {
        if self.refs.fetch_sub(1, Ordering::Relaxed) == 1 {
            unsafe {
                drop(Box::from_raw(self as *const TaskBatch as *mut TaskBatch));
            }
        }
    }
}
