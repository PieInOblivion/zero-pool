use std::sync::{Condvar, Mutex, atomic::Ordering};

use crate::padded_type::PaddedAtomicUsize;

pub struct WaitGate {
    waiters: PaddedAtomicUsize,
    mutex: Mutex<()>,
    cv: Condvar,
}

impl WaitGate {
    pub fn new() -> Self {
        Self {
            waiters: PaddedAtomicUsize::new(0),
            mutex: Mutex::new(()),
            cv: Condvar::new(),
        }
    }

    // block until predicate becomes true
    pub fn wait_until<P: Fn() -> bool>(&self, predicate: P) {
        if predicate() {
            return;
        }
        let mut guard = self.mutex.lock().unwrap();
        if predicate() {
            return;
        }

        // publish intent to sleep before actually waiting
        self.waiters.fetch_add(1, Ordering::Relaxed);
        while !predicate() {
            guard = self.cv.wait(guard).unwrap();
        }
        self.waiters.fetch_sub(1, Ordering::Relaxed);
    }

    // fast path notify: only if some thread registered as waiting.
    pub fn notify_all_if_waiters(&self) {
        if self.waiters.load(Ordering::Relaxed) == 0 {
            return;
        }
        let _g = self.mutex.lock().unwrap();
        self.cv.notify_all();
    }

    pub fn notify_all(&self) {
        let _g = self.mutex.lock().unwrap();
        self.cv.notify_all();
    }
}
