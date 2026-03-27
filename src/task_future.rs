use std::ptr::NonNull;
use std::time::Duration;

use crate::task_batch::TaskBatch;

/// A future that tracks completion of submitted tasks
///
/// `TaskFuture` provides both blocking and non-blocking ways to wait for
/// task completion. Tasks can be checked for completion, waited on
/// indefinitely, or waited on with a timeout.
///
/// `TaskFuture` is cheaply cloneable. However, it captures the thread handle
/// of the thread that created it.
///
/// If sharing the future with other threads, `is_complete()` is safe to call
/// from anywhere.
///
/// **Important:** `wait()` and `wait_timeout()` **must** be called from the
/// same thread that created the `TaskFuture`. Calling these methods from a
/// different thread will panic in debug builds and may cause the calling thread
/// to hang indefinitely in release builds.
///
pub struct TaskFuture {
    batch: NonNull<TaskBatch>,
}

impl TaskFuture {
    pub(crate) unsafe fn new(batch: *mut TaskBatch) -> Self {
        TaskFuture {
            batch: unsafe { NonNull::new_unchecked(batch) },
        }
    }

    /// Check if all tasks are complete without blocking
    ///
    /// Returns `true` if all tasks have finished execution.
    /// This is a non-blocking operation using atomic loads.
    pub fn is_complete(&self) -> bool {
        unsafe { self.batch.as_ref().is_complete() }
    }

    /// Wait for all tasks to complete
    ///
    /// First checks completion with an atomic load; if incomplete, parks the thread that sent the work.
    pub fn wait(&self) {
        unsafe { self.batch.as_ref().wait() }
    }

    /// Wait for all tasks to complete with a timeout
    ///
    /// First checks completion with an atomic load; if incomplete, parks the thread that sent the work.
    /// Returns `true` if all tasks completed within the timeout,
    /// `false` if the timeout was reached first.
    pub fn wait_timeout(&self, timeout: Duration) -> bool {
        unsafe { self.batch.as_ref().wait_timeout(timeout) }
    }
}

impl Clone for TaskFuture {
    fn clone(&self) -> Self {
        unsafe {
            self.batch.as_ref().retain();
        }
        TaskFuture { batch: self.batch }
    }
}

impl Drop for TaskFuture {
    fn drop(&mut self) {
        unsafe {
            self.batch.as_ref().release();
        }
    }
}
