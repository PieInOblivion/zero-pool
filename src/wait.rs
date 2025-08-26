use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Condvar, Mutex};

use crate::padded_type::PaddedAtomicUsize;

pub struct SleepNotifier {
    sleepers: PaddedAtomicUsize,
    mutex: Mutex<()>,
    condvar: Condvar,
    worker_count: usize,
}

impl SleepNotifier {
    pub fn new(worker_count: usize) -> Self {
        Self {
            sleepers: PaddedAtomicUsize::new(0),
            mutex: Mutex::new(()),
            condvar: Condvar::new(),
            worker_count,
        }
    }

    pub fn fast_notify(&self) {
        let sleepers = self.sleepers.load(Ordering::Relaxed);

        if sleepers == 0 || sleepers == self.worker_count {
            self.condvar.notify_all();
            return;
        }

        self.safe_notify();
    }

    pub fn safe_notify(&self) {
        let _g = self.mutex.lock().unwrap();
        self.condvar.notify_all();
    }

    /// returns true if shutdown triggered, false if predicate satisfied.
    pub fn wait_for<P>(&self, predicate: P, shutdown: &AtomicBool) -> bool
    where
        P: Fn() -> bool,
    {
        // quick checks without touching sleepers accounting
        if shutdown.load(Ordering::Relaxed) {
            return true;
        }
        if predicate() {
            return false;
        }

        // Declare intent to sleep so producers will lock.
        // Release prevents the increment moving past the mutex lock, ensuring a producer
        // that observes 0 truly saw no thread that had begun the sleep transition.
        self.sleepers.fetch_add(1, Ordering::Relaxed);
        let mut guard = self.mutex.lock().unwrap();
        loop {
            if shutdown.load(Ordering::Relaxed) {
                self.sleepers.fetch_sub(1, Ordering::Relaxed);
                return true;
            }
            if predicate() {
                self.sleepers.fetch_sub(1, Ordering::Relaxed);
                return false;
            }
            guard = self.condvar.wait(guard).unwrap();
        }
    }
}
