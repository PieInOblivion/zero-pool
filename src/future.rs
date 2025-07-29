use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::{Arc, Condvar, Mutex};
use std::time::Duration;

// public work future with arc wrapped fields
#[derive(Clone)]
pub struct WorkFuture {
    remaining: Arc<AtomicUsize>,
    state: Arc<(Mutex<()>, Condvar)>,
}

impl WorkFuture {
    // create a new work future for the given number of tasks
    pub fn new(task_count: usize) -> Self {
        WorkFuture {
            remaining: Arc::new(AtomicUsize::new(task_count)),
            state: Arc::new((Mutex::new(()), Condvar::new())),
        }
    }

    // check if all tasks are complete
    pub fn is_complete(&self) -> bool {
        self.remaining.load(Ordering::Acquire) == 0
    }

    // wait for all tasks to complete
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

    // wait for all tasks with timeout
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

    // get remaining task count
    pub fn remaining_count(&self) -> usize {
        self.remaining.load(Ordering::Acquire)
    }

    // complete one task, decrements counter and notifies if all done
    #[inline]
    pub fn complete_one(&self) {
        let remaining_count = self.remaining.fetch_sub(1, Ordering::AcqRel);

        // if this was the last task, notify waiters
        if remaining_count == 1 {
            let (_lock, cvar) = &*self.state;
            cvar.notify_all();
        }
    }
}
