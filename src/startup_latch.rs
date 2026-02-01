use std::sync::atomic::{AtomicUsize, Ordering};
use std::thread::{self, Thread};

pub struct StartupLatch {
    count: AtomicUsize,
    main_thread: Thread,
}

impl StartupLatch {
    pub fn new(count: usize) -> Self {
        Self {
            count: AtomicUsize::new(count),
            main_thread: thread::current(),
        }
    }

    pub fn decrement(&self) {
        if self.count.fetch_sub(1, Ordering::Release) == 1 {
            self.main_thread.unpark();
        }
    }

    pub fn wait(&self) {
        while self.count.load(Ordering::Acquire) > 0 {
            thread::park();
        }
    }
}
