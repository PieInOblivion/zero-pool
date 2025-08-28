use crate::padded_type::PaddedAtomicUsize;
use std::cell::UnsafeCell;
use std::mem::MaybeUninit;
use std::sync::atomic::{AtomicBool, Ordering};
use std::thread::{self, Thread};

pub struct Waiter {
    // hot atomics first to keep them on separate cache lines and away from other fields
    registered: PaddedAtomicUsize,
    top: PaddedAtomicUsize,
    // treiber lifo stack: per-worker `next` pointers (padded per entry)
    stack_next: Box<[PaddedAtomicUsize]>,
    // per-worker thread slots. each worker stores its Thread handle here once at startup.
    // `ready` is release/set by the worker; readers use acquire to see the stored handle.
    worker_threads: Box<[ThreadSlot]>,
}

#[repr(align(64))]
struct ThreadSlot {
    ready: AtomicBool,
    thread: UnsafeCell<MaybeUninit<Thread>>,
}

unsafe impl Sync for ThreadSlot {}

impl ThreadSlot {
    const fn new() -> Self {
        ThreadSlot {
            ready: AtomicBool::new(false),
            thread: UnsafeCell::new(MaybeUninit::uninit()),
        }
    }

    fn store_thread(&self, t: Thread) {
        // write thread handle then publish with release so notifiers see it.
        unsafe {
            (*self.thread.get()).write(t);
        }
        self.ready.store(true, Ordering::Release);
    }

    fn is_ready(&self) -> bool {
        self.ready.load(Ordering::Acquire)
    }

    fn get_thread_ref(&self) -> &Thread {
        unsafe { (&*self.thread.get()).assume_init_ref() }
    }

    unsafe fn drop_if_init(&self) {
        // drop the stored Thread if the slot was initialized.
        if self.ready.load(Ordering::Acquire) {
            unsafe {
                std::ptr::drop_in_place(
                    self.thread.get() as *mut MaybeUninit<Thread> as *mut Thread
                );
            }
        }
    }
}

impl Waiter {
    pub fn new(worker_count: usize) -> Self {
        // choose power-of-two capacity >= worker_count (used by other allocations)
        let mut cap = 1usize;
        while cap < worker_count {
            cap <<= 1;
        }

        let worker_threads_vec: Vec<ThreadSlot> =
            (0..worker_count).map(|_| ThreadSlot::new()).collect();
        let worker_threads = worker_threads_vec.into_boxed_slice();

        let next_vec: Vec<PaddedAtomicUsize> = (0..worker_count)
            .map(|_| PaddedAtomicUsize::new(usize::MAX))
            .collect();
        let stack_next = next_vec.into_boxed_slice();

        Waiter {
            registered: PaddedAtomicUsize::new(0),
            top: PaddedAtomicUsize::new(usize::MAX),
            stack_next,
            worker_threads,
        }
    }

    /// register current thread into the per-worker slot.
    /// worker must call this once before parking. the store uses release ordering.
    pub fn register_current_thread(&self, worker_id: usize) {
        // Store the current thread handle into the preallocated slot.
        self.worker_threads[worker_id].store_thread(thread::current());
        // count this registration
        self.registered.fetch_add(1, Ordering::AcqRel);
    }

    pub fn registered_count(&self) -> usize {
        self.registered.load(Ordering::Acquire)
    }

    /// push id onto the treiber stack. single-atomic fast-path.
    /// we write next[id] (relaxed) then CAS top -> id (acqrel). this ordering is
    /// enough because the worker has already published its thread handle.
    fn enqueue_id(&self, id: usize) -> bool {
        loop {
            let head = self.top.load(Ordering::Acquire);
            // set next[id] = head (relaxed; synchronization happens on the CAS)
            self.stack_next[id].store(head, Ordering::Relaxed);
            // try to swing top -> id; acqrel so later pop sees next pointer.
            if self
                .top
                .compare_exchange_weak(head, id, Ordering::AcqRel, Ordering::Acquire)
                .is_ok()
            {
                return true;
            }
            // CAS failed, retry
            std::hint::spin_loop();
        }
    }

    /// pop id from the treiber stack. returns None when empty.
    /// on success the popped id's next pointer is visible due to acqrel CAS.
    fn dequeue_id(&self) -> Option<usize> {
        loop {
            let head = self.top.load(Ordering::Acquire);
            if head == usize::MAX {
                return None;
            }
            // load next with acquire so we see any writes to next by the pusher.
            let next = self.stack_next[head].load(Ordering::Acquire);
            if self
                .top
                .compare_exchange(head, next, Ordering::AcqRel, Ordering::Acquire)
                .is_ok()
            {
                return Some(head);
            }
            // CAS failed, retry
        }
    }

    /// Notify up to `count` waiters (bounded by number of workers). If `count` is larger than
    /// the worker capacity, it will notify as many as available.
    pub fn notify(&self, mut count: usize) {
        let cap = self.worker_threads.len();
        if count > cap {
            count = cap;
        }

        while count > 0 {
            if let Some(id) = self.dequeue_id() {
                if id == usize::MAX {
                    continue;
                }
                let slot = &self.worker_threads[id];
                if slot.is_ready() {
                    // worker registered, safe to unpark
                    slot.get_thread_ref().unpark();
                } else {
                    // worker hasn't registered yet. try to push the id back a few times
                    // so it can be observed later once the worker completes registration.
                    let mut requeued = false;
                    for _ in 0..3 {
                        if self.enqueue_id(id) {
                            requeued = true;
                            break;
                        }
                        std::hint::spin_loop();
                    }
                    if !requeued {
                        // skip this id for now
                    }
                }
                count -= 1;
            } else {
                break;
            }
        }
    }

    /// Worker parks itself after enqueuing its id.
    pub fn wait_for<P>(&self, predicate: P, shutdown: &AtomicBool, worker_id: usize) -> bool
    where
        P: Fn() -> bool,
    {
        if predicate() {
            return false;
        }

        loop {
            if shutdown.load(Ordering::Relaxed) {
                return true;
            }
            if predicate() {
                return false;
            }

            if self.enqueue_id(worker_id) {
                // park until unparked. park/unpark handles the case where unpark
                // happens before park: park returns immediately if unparked earlier.
                thread::park();

                if shutdown.load(Ordering::Relaxed) {
                    return true;
                }
                if predicate() {
                    return false;
                }
                continue; // spurious wakeup or not yet ready, loop again
            }

            // fallback (shouldn't happen with treiber stack): yield and retry
            thread::yield_now();
        }
    }
}

impl Drop for Waiter {
    fn drop(&mut self) {
        for at in &self.worker_threads {
            unsafe {
                at.drop_if_init();
            }
        }
    }
}
