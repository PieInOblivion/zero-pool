use std::sync::{Arc, Condvar, Mutex};
use std::time::Duration;

// Future that signals task completion
#[derive(Clone)]
pub struct WorkFuture {
    pub state: Arc<(Mutex<bool>, Condvar)>,
}

impl WorkFuture {
    pub fn new() -> Self {
        WorkFuture {
            state: Arc::new((Mutex::new(false), Condvar::new())),
        }
    }

    // Wait for the task to complete
    pub fn wait(&self) {
        let (lock, cvar) = &*self.state;
        let mut completed = lock.lock().unwrap();
        while !*completed {
            completed = cvar.wait(completed).unwrap();
        }
    }

    // Wait with timeout
    pub fn wait_timeout(&self, timeout: Duration) -> bool {
        let (lock, cvar) = &*self.state;
        let mut completed = lock.lock().unwrap();
        while !*completed {
            let (guard, timeout_result) = cvar.wait_timeout(completed, timeout).unwrap();
            completed = guard;
            if timeout_result.timed_out() {
                return false;
            }
        }
        true
    }

    // Check if complete
    pub fn is_complete(&self) -> bool {
        *self.state.0.lock().unwrap()
    }

    // Mark as complete (internal fn only)
    pub(crate) fn complete(&self) {
        let (lock, cvar) = &*self.state;
        *lock.lock().unwrap() = true;
        cvar.notify_all();
    }
}

// Batch of futures for multiple tasks
pub struct WorkFutureBatch {
    pub futures: Vec<WorkFuture>,
}

impl WorkFutureBatch {
    pub fn new(futures: Vec<WorkFuture>) -> Self {
        WorkFutureBatch { futures }
    }

    // Check if all tasks are complete
    pub fn is_complete(&self) -> bool {
        self.futures.iter().all(|f| f.is_complete())
    }

    // Wait for all tasks to complete
    pub fn wait(self) {
        for future in self.futures {
            future.wait();
        }
    }

    // Wait for all tasks with timeout
    pub fn wait_timeout(self, timeout: Duration) -> bool {
        let start = std::time::Instant::now();
        for future in self.futures {
            let remaining = timeout.saturating_sub(start.elapsed());
            if !future.wait_timeout(remaining) {
                return false;
            }
        }
        true
    }
}
