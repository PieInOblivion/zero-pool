use crate::padded_type::PaddedAtomicPtr;
use std::cell::UnsafeCell;
use std::mem::MaybeUninit;
use std::ptr;
use std::sync::atomic::AtomicPtr;
use std::sync::atomic::{AtomicBool, Ordering};
use std::thread::{self, Thread};

pub struct Waiter {
    // Michael-Scott queue using heap nodes: head/tail point to Node
    head: PaddedAtomicPtr<Node>,
    tail: PaddedAtomicPtr<Node>,
    // worker thread slots. each worker stores its handle here once at startup
    // ready is release/set by the worker. readers use acquire to see the stored handle
    worker_threads: Box<[ThreadSlot]>,
}

struct ThreadSlot {
    // store the thread handle in-place once at registration
    thread: UnsafeCell<MaybeUninit<Thread>>,
}

struct Node {
    id: usize,
    next: AtomicPtr<Node>,
}

unsafe impl Sync for ThreadSlot {}

impl ThreadSlot {
    fn new() -> Self {
        ThreadSlot {
            thread: UnsafeCell::new(MaybeUninit::uninit()),
        }
    }

    fn store_thread(&self, t: Thread) {
        unsafe {
            (*self.thread.get()).write(t);
        }
    }

    fn get_thread_ref(&self) -> &Thread {
        unsafe { (&*self.thread.get()).assume_init_ref() }
    }

    unsafe fn drop_if_init(&self) {
        // Assume initialized (registration happens at worker spawn before
        // pool creation returns). Drop the stored Thread.
        unsafe {
            std::ptr::drop_in_place(self.thread.get() as *mut MaybeUninit<Thread> as *mut Thread);
        }
    }
}

impl Waiter {
    pub fn new(worker_count: usize) -> Self {
        // choose power-of-two capacity >= worker_count
        let mut cap = 1usize;
        while cap < worker_count {
            cap <<= 1;
        }

        let worker_threads_vec: Vec<ThreadSlot> =
            (0..worker_count).map(|_| ThreadSlot::new()).collect();
        let worker_threads = worker_threads_vec.into_boxed_slice();

        // create sentinel node for M&S queue
        let sentinel = Box::into_raw(Box::new(Node {
            id: usize::MAX,
            next: AtomicPtr::new(ptr::null_mut()),
        }));

        Waiter {
            head: PaddedAtomicPtr::new(sentinel),
            tail: PaddedAtomicPtr::new(sentinel),
            worker_threads,
        }
    }
    /// register current thread into the per worker slot
    /// worker must call this once before parking
    pub fn register_current_thread(&self, worker_id: usize) {
        // store the thread handle into the preallocated slot
        self.worker_threads[worker_id].store_thread(thread::current());
    }

    fn enqueue_id(&self, id: usize) -> bool {
        // canonical M&S enqueue with heap node per enqueue
        let node = Box::into_raw(Box::new(Node {
            id,
            next: AtomicPtr::new(ptr::null_mut()),
        }));
        loop {
            let tail = self.tail.load(Ordering::Acquire);
            let tail_next = unsafe { (*tail).next.load(Ordering::Acquire) };
            if !tail_next.is_null() {
                // tail behind, try to advance
                let _ = self.tail.compare_exchange_weak(
                    tail,
                    tail_next,
                    Ordering::AcqRel,
                    Ordering::Acquire,
                );
                continue;
            }
            // try to link node at tail.next
            if unsafe {
                (*tail)
                    .next
                    .compare_exchange_weak(
                        ptr::null_mut(),
                        node,
                        Ordering::AcqRel,
                        Ordering::Acquire,
                    )
                    .is_ok()
            } {
                // advance tail to new node (best-effort)
                let _ = self.tail.compare_exchange_weak(
                    tail,
                    node,
                    Ordering::AcqRel,
                    Ordering::Acquire,
                );
                return true;
            }
            // CAS failed, retry
        }
    }

    fn dequeue_id(&self) -> Option<usize> {
        loop {
            let head = self.head.load(Ordering::Acquire);
            let tail = self.tail.load(Ordering::Acquire);
            let head_next = unsafe { (*head).next.load(Ordering::Acquire) };
            if head_next.is_null() {
                return None;
            }
            if head == tail {
                // tail behind, try to advance
                let _ = self.tail.compare_exchange_weak(
                    tail,
                    head_next,
                    Ordering::AcqRel,
                    Ordering::Acquire,
                );
                continue;
            }
            // try to advance head
            if self
                .head
                .compare_exchange_weak(head, head_next, Ordering::AcqRel, Ordering::Acquire)
                .is_ok()
            {
                // read id and free old head node
                let id = unsafe { (*head_next).id };
                // free old sentinel/head
                unsafe {
                    drop(Box::from_raw(head));
                }
                return Some(id);
            }
            // otherwise retry
        }
    }

    /// Notify up to `count` waiters (bounded by number of workers). If `count` is larger than
    /// the worker capacity, it will notify as many as available.
    pub fn notify(&self, mut count: usize) {
        // Special shutdown fast-path: if caller passed usize::MAX we should
        // unpark all worker threads regardless of queue state. Do this before
        // clamping `count` to avoid losing the sentinel value.
        if count == usize::MAX {
            for slot in self.worker_threads.iter() {
                slot.get_thread_ref().unpark();
            }
            return;
        }

        let cap = self.worker_threads.len();
        if count > cap {
            count = cap;
        }

        while count > 0 {
            match self.dequeue_id() {
                Some(id) => {
                    if id == usize::MAX {
                        continue;
                    }
                    let slot = &self.worker_threads[id];
                    // If the worker stored a Thread handle, unpark it. We rely on
                    // publish-then-enqueue ordering so a dequeued id implies the
                    // thread handle is visible to us.
                    slot.get_thread_ref().unpark();
                    count -= 1;
                }
                None => break,
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
            if shutdown.load(Ordering::Acquire) {
                return true;
            }
            if predicate() {
                return false;
            }

            // worker must have called `register_current_thread` earlier to
            // publish its Thread. We no longer attempt lazy registration here.
            // publish thread handle (assumed already stored at registration time)
            // then push id and park. With correct ordering enqueue->dequeue
            // synchronizes visibility of the stored Thread handle.
            if self.enqueue_id(worker_id) {
                // park until unparked. park/unpark handles the case where unpark
                // happens before park: park returns immediately if unparked earlier.
                thread::park();

                if shutdown.load(Ordering::Acquire) {
                    return true;
                }
                if predicate() {
                    return false;
                }
                continue; // spurious wakeup or not yet ready, loop again
            }
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
