use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::{Arc, Condvar, Mutex};
use std::time::Duration;

use crate::padded_type::PaddedType;

/// Inner state shared between all clones of a TaskFuture
struct TaskFutureInner {
    remaining: PaddedType<AtomicUsize>,
    lock: Mutex<()>,
    cvar: Condvar,
}

/// A future that tracks completion of submitted tasks
///
/// `TaskFuture` provides both blocking and non-blocking ways to wait for
/// task completion. Tasks can be checked for completion, waited on
/// indefinitely, or waited on with a timeout.
///
/// `TaskFuture` is cheaply cloneable and can be shared across threads.
/// You can drop the future immediately after submission - tasks will
/// still complete as the task batch holds its own reference.
#[derive(Clone)]
pub struct TaskFuture(Arc<TaskFutureInner>);

impl TaskFuture {
    // create a new work future for the given number of tasks
    pub(crate) fn new(task_count: usize) -> Self {
        TaskFuture(Arc::new(TaskFutureInner {
            remaining: PaddedType::new(AtomicUsize::new(task_count)),
            lock: Mutex::new(()),
            cvar: Condvar::new(),
        }))
    }

    /// Check if all tasks are complete without blocking
    ///
    /// Returns `true` if all tasks have finished execution.
    /// This is a non-blocking operation using atomic loads.
    pub fn is_complete(&self) -> bool {
        self.0.remaining.load(Ordering::Acquire) == 0
    }

    /// Wait for all tasks to complete
    ///
    /// First checks completion with an atomic load; if incomplete, it locks the mutex.
    pub fn wait(self) {
        if self.is_complete() {
            return;
        }

        let guard = self.0.lock.lock().unwrap();

        let _guard = self
            .0
            .cvar
            .wait_while(guard, |_| !self.is_complete())
            .unwrap();
    }

    /// Wait for all tasks to complete with a timeout
    ///
    /// First checks completion with an atomic load; if incomplete, it locks the mutex.
    /// Returns `true` if all tasks completed within the timeout,
    /// `false` if the timeout was reached first.
    pub fn wait_timeout(self, timeout: Duration) -> bool {
        if self.is_complete() {
            return true;
        }

        let guard = self.0.lock.lock().unwrap();

        !self
            .0
            .cvar
            .wait_timeout_while(guard, timeout, |_| !self.is_complete())
            .unwrap()
            .1
            .timed_out()
    }

    // completes multiple tasks, decrements counter and notifies if all done
    pub(crate) fn complete_many(&self, count: usize) -> bool {
        let remaining_count = self.0.remaining.fetch_sub(count, Ordering::Release);

        if remaining_count != count {
            return false;
        }

        let _guard = self.0.lock.lock().unwrap();
        self.0.cvar.notify_all();
        true
    }
}
