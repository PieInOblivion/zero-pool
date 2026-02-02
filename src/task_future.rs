use crate::atomic_latch::AtomicLatch;
use std::time::Duration;

/// A future that tracks completion of submitted tasks
///
/// `TaskFuture` provides both blocking and non-blocking ways to wait for
/// task completion. Tasks can be checked for completion, waited on
/// indefinitely, or waited on with a timeout.
///
/// `TaskFuture` is cheaply cloneable. However, it captures the thread handle
/// of the thread that created it, and `wait`/`wait_timeout` should only be
/// called from that same thread. If sharing the future to other threads,
/// is_complete() will still work.
#[derive(Clone)]
pub struct TaskFuture(AtomicLatch);

impl TaskFuture {
    // create a new work future for the given number of tasks.
    // padding 'remaining' overall tests like a net negative for performance,
    // this is however inconclusive
    pub(crate) fn new(task_count: usize) -> Self {
        TaskFuture(AtomicLatch::new(task_count))
    }

    /// Check if all tasks are complete without blocking
    ///
    /// Returns `true` if all tasks have finished execution.
    /// This is a non-blocking operation using atomic loads.
    pub fn is_complete(&self) -> bool {
        self.0.is_complete()
    }

    /// Wait for all tasks to complete
    ///
    /// First checks completion with an atomic load; if incomplete, parks the thread that sent the work.
    pub fn wait(self) {
        self.0.wait();
    }

    /// Wait for all tasks to complete with a timeout
    ///
    /// First checks completion with an atomic load; if incomplete, parks the thread that sent the work.
    /// Returns `true` if all tasks completed within the timeout,
    /// `false` if the timeout was reached first.
    pub fn wait_timeout(self, timeout: Duration) -> bool {
        self.0.wait_timeout(timeout)
    }

    // completes multiple tasks, decrements counter and notifies if all done
    pub(crate) fn complete_many(&self, count: usize) -> bool {
        self.0.decrement(count)
    }
}
