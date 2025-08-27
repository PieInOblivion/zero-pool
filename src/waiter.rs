use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Condvar, Mutex};

pub struct Waiter {
    mutex: Mutex<()>,
    condvar: Condvar,
}

impl Waiter {
    pub fn new() -> Self {
        Self {
            mutex: Mutex::new(()),
            condvar: Condvar::new(),
        }
    }

    pub fn notify(&self) {
        self.condvar.notify_all();
    }

    pub fn wait_for<P>(&self, predicate: P, shutdown: &AtomicBool) -> bool
    where
        P: Fn() -> bool,
    {
        let mut guard = self.mutex.lock().unwrap();

        loop {
            if predicate() {
                return false;
            }

            if shutdown.load(Ordering::Relaxed) {
                return true;
            }

            guard = self.condvar.wait(guard).unwrap();
        }
    }
}
