use std::ptr::NonNull;
use std::sync::atomic::{AtomicPtr, AtomicUsize, Ordering};
use std::thread::{self, Thread};
use std::time::{Duration, Instant};

use crate::{TaskFnPointer, TaskParamPointer, padded_type::PaddedType};

pub struct TaskBatch {
    next_byte_offset: PaddedType<AtomicUsize>,
    // pointer arithmetic instead of usize address math to preserve pointer provenance
    params_ptr: TaskParamPointer,
    param_stride: usize,
    params_total_bytes: usize,
    pub task_fn_ptr: TaskFnPointer,
    pub next: AtomicPtr<TaskBatch>,

    count: AtomicUsize,
    owner_thread: Thread,
    viewers: AtomicUsize,
}

impl TaskBatch {
    pub(crate) fn new<T>(task_fn_ptr: TaskFnPointer, params: &[T]) -> Self {
        let initial_viewers = if params.is_empty() { 1 } else { 2 };

        TaskBatch {
            next_byte_offset: PaddedType::new(AtomicUsize::new(0)),
            params_ptr: NonNull::from(params).cast(),
            param_stride: std::mem::size_of::<T>(),
            params_total_bytes: std::mem::size_of_val(params),
            task_fn_ptr,
            next: AtomicPtr::new(std::ptr::null_mut()),
            count: AtomicUsize::new(params.len()),
            owner_thread: thread::current(),
            viewers: AtomicUsize::new(initial_viewers),
        }
    }

    pub(crate) fn claim_next_param(&self) -> Option<TaskParamPointer> {
        let byte_offset = self
            .next_byte_offset
            .fetch_add(self.param_stride, Ordering::Relaxed);

        if byte_offset >= self.params_total_bytes {
            return None;
        }
        unsafe { Some(self.params_ptr.add(byte_offset)) }
    }

    pub(crate) fn has_unclaimed_tasks(&self) -> bool {
        self.next_byte_offset.load(Ordering::Relaxed) < self.params_total_bytes
    }

    pub(crate) fn viewers_increment(&self) {
        self.viewers.fetch_add(1, Ordering::Release);
    }

    pub(crate) fn viewers_decrement(&self) {
        self.viewers.fetch_sub(1, Ordering::Release);
    }

    pub(crate) fn viewers_count(&self) -> usize {
        self.viewers.load(Ordering::Acquire)
    }

    /// Check if all tasks are complete without blocking
    ///
    /// Returns `true` if all tasks have finished execution.
    /// This is a non-blocking operation using atomic loads.
    pub fn is_complete(&self) -> bool {
        self.count.load(Ordering::Acquire) == 0
    }

    /// Wait for all tasks to complete
    ///
    /// First checks completion with an atomic load; if incomplete, parks the thread that sent the work.
    pub fn wait(&self) {
        debug_assert_eq!(
            self.owner_thread.id(),
            thread::current().id(),
            "TaskFuture::wait() must be called from the thread that created it."
        );

        while !self.is_complete() {
            thread::park();
        }
    }

    /// Wait for all tasks to complete with a timeout
    ///
    /// First checks completion with an atomic load; if incomplete, parks the thread that sent the work.
    /// Returns `true` if all tasks completed within the timeout,
    /// `false` if the timeout was reached first.
    pub fn wait_timeout(&self, timeout: Duration) -> bool {
        debug_assert_eq!(
            self.owner_thread.id(),
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

    // completes multiple tasks, decrements counter and notifies if all done
    pub(crate) fn complete_many(&self, completed: usize) -> bool {
        if self.count.fetch_sub(completed, Ordering::Release) == completed {
            self.owner_thread.unpark();
            true
        } else {
            false
        }
    }
}
