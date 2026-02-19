use std::sync::atomic::{AtomicUsize, Ordering};
use std::thread::{self, Thread};

pub struct StartupLatch {
    count: AtomicUsize,
}

impl StartupLatch {
    pub fn new(count: usize) -> Self {
        Self {
            count: AtomicUsize::new(count),
        }
    }

    /// creates a sendable signaler that can be passed to worker threads
    pub fn signaler(&self) -> LatchSignaler {
        LatchSignaler {
            // raw pointer to satisfy miri's strict provenance rules
            ptr: &self.count as *const AtomicUsize,
            waiter: thread::current(),
        }
    }

    pub fn wait(&self) {
        while self.count.load(Ordering::Acquire) != 0 {
            thread::park();
        }
    }
}

pub struct LatchSignaler {
    ptr: *const AtomicUsize,
    waiter: Thread,
}

// we guarantee the StartupLatch outlives the LatchSignaler because
// the main thread calls StartupLatch::wait() until all
// outstanding signalers have dropped the counter to zero
unsafe impl Send for LatchSignaler {}

impl LatchSignaler {
    pub fn signal(self) {
        let count = unsafe { &*self.ptr };

        if count.fetch_sub(1, Ordering::Release) == 1 {
            self.waiter.unpark();
        }
    }
}
