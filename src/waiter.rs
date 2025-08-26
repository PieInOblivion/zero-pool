use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Condvar, Mutex};

use crate::padded_type::PaddedAtomicUsize;

pub struct Waiter {
    epoch: PaddedAtomicUsize,
    mutex: Mutex<()>,
    condvar: Condvar,
}

impl Waiter {
    pub fn new() -> Self {
        Self {
            epoch: PaddedAtomicUsize::new(0),
            mutex: Mutex::new(()),
            condvar: Condvar::new(),
        }
    }

    pub fn notify_work(&self) {
        self.epoch.fetch_add(1, Ordering::Relaxed);
        self.condvar.notify_all();
    }

    pub fn notify_shutdown(&self) {
        self.epoch.fetch_add(1, Ordering::Release);
        self.condvar.notify_all();
    }

    pub fn wait_for<P>(&self, predicate: P, shutdown: &AtomicBool) -> bool
    where
        P: Fn() -> bool,
    {
        loop {
            if predicate() {
                return false;
            }

            let snap = self.epoch.load(Ordering::Relaxed);

            let guard = self.mutex.lock().unwrap();

            if predicate() {
                return false;
            }

            if shutdown.load(Ordering::Relaxed) {
                return true;
            }

            if self.epoch.load(Ordering::Relaxed) != snap {
                return false;
            }

            let _after = self.condvar.wait(guard).unwrap();
        }
    }
}
