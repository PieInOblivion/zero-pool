use std::sync::atomic::Ordering;
use std::sync::{Arc, Condvar, Mutex};
use std::time::Duration;

use crate::padded_type::PaddedAtomicUsize;

/// A future that tracks completion of submitted tasks
///
/// `TaskFuture` provides both blocking and non-blocking ways to wait for
/// task completion, with efficient condition variable notification.
/// Tasks can be checked for completion, waited on indefinitely, or
/// waited on with a timeout.
#[derive(Clone)]
pub struct TaskFuture {
    remaining: Arc<PaddedAtomicUsize>,
    state: Arc<(Mutex<()>, Condvar)>,
}

impl TaskFuture {
    // create a new work future for the given number of tasks
    pub(crate) fn new(task_count: usize) -> Self {
        TaskFuture {
            remaining: Arc::new(PaddedAtomicUsize::new(task_count)),
            state: Arc::new((Mutex::new(()), Condvar::new())),
        }
    }

    /// Check if all tasks are complete without blocking
    ///
    /// Returns `true` if all tasks have finished execution.
    /// This is a non-blocking operation using atomic loads.
    pub fn is_complete(&self) -> bool {
        self.remaining.load(Ordering::Acquire) == 0
    }

    /// Wait for all tasks to complete
    ///
    /// This method blocks the current thread until all tasks finish.
    /// It uses efficient condition variable notification to minimize
    /// CPU usage while waiting.
    pub fn wait(self) {
        if self.is_complete() {
            return;
        }

        let (lock, cvar) = &*self.state;
        let mut guard = lock.lock().unwrap();

        while !self.is_complete() {
            guard = cvar.wait(guard).unwrap();
        }
    }

    /// Wait for all tasks to complete with a timeout
    ///
    /// Returns `true` if all tasks completed within the timeout,
    /// `false` if the timeout was reached first.
    pub fn wait_timeout(self, timeout: Duration) -> bool {
        if self.is_complete() {
            return true;
        }

        let (lock, cvar) = &*self.state;
        let mut guard = lock.lock().unwrap();

        while !self.is_complete() {
            let (new_guard, timeout_result) = cvar.wait_timeout(guard, timeout).unwrap();
            guard = new_guard;
            if timeout_result.timed_out() {
                return self.is_complete();
            }
        }
        true
    }

    /// Get the approximate number of remaining incomplete tasks
    ///
    /// This provides a way to monitor progress of task batches.
    /// The count is approximate due to relaxed atomic ordering.
    /// Not for precise synchronization.
    pub fn remaining_count(&self) -> usize {
        self.remaining.load(Ordering::Relaxed)
    }

    // completes multiple tasks, decrements counter and notifies if all done
    pub(crate) fn complete_many(&self, count: usize) {
        let remaining_count = self.remaining.fetch_sub(count, Ordering::Release);

        // if this completed the last tasks, notify waiters
        if remaining_count == count {
            let (lock, cvar) = &*self.state;
            let _guard = lock.lock().unwrap();
            cvar.notify_all();
        }
    }
}
