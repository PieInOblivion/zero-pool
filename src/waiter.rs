use crate::padded_type::PaddedAtomicBool;
use std::cell::UnsafeCell;
use std::sync::atomic::Ordering;
use std::thread::{self, Thread};

pub struct Waiter {
    threads: Box<[UnsafeCell<Option<Thread>>]>,
    parked: Box<[PaddedAtomicBool]>,
}

unsafe impl Sync for Waiter {}

impl Waiter {
    pub fn new(worker_count: usize) -> Self {
        let mut t = Vec::with_capacity(worker_count);
        for _ in 0..worker_count {
            t.push(UnsafeCell::new(None));
        }

        let mut p = Vec::with_capacity(worker_count);
        for _ in 0..worker_count {
            p.push(PaddedAtomicBool::new(false));
        }

        Waiter {
            threads: t.into_boxed_slice(),
            parked: p.into_boxed_slice(),
        }
    }

    pub fn register_current_thread(&self, worker_id: usize) {
        unsafe {
            *self.threads[worker_id].get() = Some(thread::current());
        }
    }

    // scan the array and attempt to unpark up to count workers
    pub fn notify(&self, count: usize) {
        let len = self.threads.len();

        let mut notified = 0usize;
        for i in 0..len {
            if notified == count {
                break;
            }

            if self.parked[i].load(Ordering::Acquire) {
                unsafe {
                    if let Some(t) = (&*self.threads[i].get()).as_ref() {
                        t.unpark();
                    }
                }
                notified += 1;
            }
        }
    }

    pub fn wait_for<P>(&self, predicate: P, worker_id: usize)
    where
        P: Fn() -> bool,
    {
        self.parked[worker_id].store(true, Ordering::Release);

        while !predicate() {
            thread::park();
        }

        self.parked[worker_id].store(false, Ordering::Release);
    }
}

impl Drop for Waiter {
    fn drop(&mut self) {
        // take ownership of any stored thread handles so they can be dropped
        for slot in self.threads.iter() {
            unsafe {
                let s = &mut *slot.get();
                let _ = s.take();
            }
        }
    }
}
