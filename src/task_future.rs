use std::sync::Arc;
use std::sync::atomic::{AtomicUsize, Ordering};
use std::thread::{self, Thread};
use std::time::{Duration, Instant};

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
#[derive(Clone)]
pub struct TaskFuture {
    count: Arc<AtomicUsize>,
    owner_thread: Thread,
}

impl TaskFuture {
    pub(crate) fn new(task_count: usize) -> Self {
        TaskFuture {
            count: Arc::new(AtomicUsize::new(task_count)),
            owner_thread: thread::current(),
        }
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
    pub(crate) fn complete_many(&self, count: usize) -> bool {
        if self.count.fetch_sub(count, Ordering::Release) == count {
            self.owner_thread.unpark();
            true
        } else {
            false
        }
    }
}
