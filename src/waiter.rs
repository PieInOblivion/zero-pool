use crate::padded_type::PaddedAtomicPtr;
use std::cell::UnsafeCell;
use std::ptr;
use std::sync::atomic::Ordering;
use std::thread::{self, Thread};

// treiber stack based waiter
pub struct Waiter {
    head: PaddedAtomicPtr<Node>,
    worker_threads: Box<[ThreadSlot]>,
}

struct ThreadSlot {
    // store the thread handle in-place once at registration
    // using Option simplifies init/drop logic
    thread: UnsafeCell<Option<Thread>>,
}

struct Node {
    id: usize,
    // not atomic, written by the owner before publishing the node
    next: *mut Node,
}

unsafe impl Sync for ThreadSlot {}

impl ThreadSlot {
    fn new() -> Self {
        ThreadSlot {
            thread: UnsafeCell::new(None),
        }
    }

    fn store_thread(&self, t: Thread) {
        unsafe {
            *self.thread.get() = Some(t);
        }
    }

    fn get_thread_ref(&self) -> &Thread {
        unsafe { (&*self.thread.get()).as_ref().unwrap() }
    }
}

impl Waiter {
    pub fn new(worker_count: usize) -> Self {
        let worker_threads_vec: Vec<ThreadSlot> =
            (0..worker_count).map(|_| ThreadSlot::new()).collect();
        let worker_threads = worker_threads_vec.into_boxed_slice();

        // empty stack initially
        Waiter {
            head: PaddedAtomicPtr::new(ptr::null_mut()),
            worker_threads,
        }
    }

    /// register current thread into the per worker slot
    /// worker must call this once before parking
    pub fn register_current_thread(&self, worker_id: usize) {
        self.worker_threads[worker_id].store_thread(thread::current());
    }

    fn push_id(&self, id: usize) {
        let node = Box::into_raw(Box::new(Node {
            id,
            next: ptr::null_mut(),
        }));

        loop {
            // relaxed load is sufficient, successful CAS with release publishes the node
            let head = self.head.load(Ordering::Relaxed);
            unsafe {
                (*node).next = head;
            }

            if self
                .head
                .compare_exchange_weak(head, node, Ordering::Release, Ordering::Relaxed)
                .is_ok()
            {
                return;
            }
        }
    }

    // returns none if empty
    fn pop_id(&self) -> Option<usize> {
        loop {
            // relaxed load, successful CAS with acquire synchronises subsequent reads
            let head = self.head.load(Ordering::Relaxed);
            if head.is_null() {
                return None;
            }

            let next = unsafe { (*head).next };

            if self
                .head
                .compare_exchange_weak(head, next, Ordering::Acquire, Ordering::Relaxed)
                .is_ok()
            {
                let id = unsafe { (*head).id };
                unsafe {
                    drop(Box::from_raw(head));
                }
                return Some(id);
            }
        }
    }

    // wake up to count waiters, waking up to as many as available
    // usize::max unparks all registered workers
    pub fn notify(&self, count: usize) {
        if count == usize::MAX {
            for slot in self.worker_threads.iter() {
                slot.get_thread_ref().unpark();
            }
            return;
        }

        let to_notify = count.min(self.worker_threads.len());
        for _ in 0..to_notify {
            match self.pop_id() {
                Some(id) => {
                    let slot = &self.worker_threads[id];
                    slot.get_thread_ref().unpark();
                }
                None => break,
            }
        }
    }

    pub fn wait_for<P>(&self, predicate: P, worker_id: usize)
    where
        P: Fn() -> bool,
    {
        while !predicate() {
            self.push_id(worker_id);
            thread::park();
        }
    }
}

impl Drop for Waiter {
    fn drop(&mut self) {
        for at in &self.worker_threads {
            unsafe {
                let slot = &mut *at.thread.get();
                let _ = slot.take();
            }
        }

        // drain remaining nodes on the stack
        while let Some(_) = self.pop_id() {}
    }
}
