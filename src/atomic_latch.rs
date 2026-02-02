use std::sync::Arc;
use std::sync::atomic::{AtomicUsize, Ordering};
use std::thread::{self, Thread};
use std::time::{Duration, Instant};

#[derive(Clone)]
pub struct AtomicLatch {
    count: Arc<AtomicUsize>,
    owner_thread: Thread,
}

impl AtomicLatch {
    pub fn new(count: usize) -> Self {
        Self {
            count: Arc::new(AtomicUsize::new(count)),
            owner_thread: thread::current(),
        }
    }

    pub fn decrement(&self, n: usize) -> bool {
        if self.count.fetch_sub(n, Ordering::Release) == n {
            self.owner_thread.unpark();
            true
        } else {
            false
        }
    }

    pub fn is_complete(&self) -> bool {
        self.count.load(Ordering::Acquire) == 0
    }

    pub fn wait(&self) {
        debug_assert_eq!(
            self.owner_thread.id(),
            thread::current().id(),
            "TaskFuture::wait() must be called from the thread that created it."
        );

        while !self.is_complete() {
            thread::park();
        }
    }

    pub fn wait_timeout(&self, timeout: Duration) -> bool {
        debug_assert_eq!(
            self.owner_thread.id(),
            thread::current().id(),
            "TaskFuture::wait_timeout() must be called from the thread that created it."
        );

        let start = Instant::now();
        loop {
            if self.is_complete() {
                return true;
            }
            let elapsed = start.elapsed();
            if elapsed >= timeout {
                return false;
            }
            thread::park_timeout(timeout - elapsed);
        }
    }
}
