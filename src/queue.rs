use crate::padded_type::PaddedType;
use crate::task_batch::TaskBatch;
use crate::{TaskFnPointer, TaskFuture, TaskParamPointer};
use std::cell::UnsafeCell;
use std::mem::MaybeUninit;
use std::ptr::NonNull;
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, AtomicPtr, Ordering};
use std::thread::{self, Thread};

pub struct Queue {
    head: PaddedType<AtomicPtr<TaskBatch>>,
    tail: PaddedType<AtomicPtr<TaskBatch>>,
    oldest: PaddedType<AtomicPtr<TaskBatch>>,
    threads_is_awake: Box<[PaddedType<AtomicBool>]>,
    threads: Box<[UnsafeCell<MaybeUninit<Thread>>]>,
    shutdown: AtomicBool,
}

// needed for 'threads'
unsafe impl Sync for Queue {}

impl Queue {
    pub fn new(worker_count: usize) -> Self {
        fn noop(_: TaskParamPointer) {}
        let anchor_node = Box::into_raw(Box::new(TaskBatch::new_anchor(noop)));

        let threads_is_awake: Box<[_]> = (0..worker_count)
            .map(|_| PaddedType::new(AtomicBool::new(true)))
            .collect();

        let threads: Box<[_]> = (0..worker_count)
            .map(|_| UnsafeCell::new(MaybeUninit::uninit()))
            .collect();

        Queue {
            head: PaddedType::new(AtomicPtr::new(anchor_node)),
            tail: PaddedType::new(AtomicPtr::new(anchor_node)),
            oldest: PaddedType::new(AtomicPtr::new(anchor_node)),
            threads_is_awake,
            threads,
            shutdown: AtomicBool::new(false),
        }
    }

    pub fn push_task_batch<T>(self: &Arc<Self>, task_fn: fn(&T), params: &[T]) -> TaskFuture {
        assert!(
            !params.is_empty(),
            "zero_pool does not accept empty task batches"
        );

        let raw_fn: TaskFnPointer = unsafe { std::mem::transmute(task_fn) };
        let queue_ptr = NonNull::from(self.as_ref());
        let new_batch = Box::into_raw(Box::new(TaskBatch::new(raw_fn, params)));

        let prev_tail = self.tail.swap(new_batch, Ordering::SeqCst);
        unsafe {
            (*prev_tail).next.store(new_batch, Ordering::Release);
        }

        self.notify(params.len());
        TaskFuture::from_batch(queue_ptr, new_batch)
    }

    pub fn get_next_batch(
        &self,
        last_incremented_on: &mut *mut TaskBatch,
    ) -> Option<(&TaskBatch, TaskParamPointer)> {
        let mut current = self.head.load(Ordering::Acquire);

        loop {
            let batch = unsafe { &*current };

            if let Some(param) = batch.claim_next_param() {
                if current != *last_incremented_on {
                    batch.viewers_increment();

                    let previous = *last_incremented_on;
                    *last_incremented_on = current;

                    if !previous.is_null() {
                        self.release_viewer(previous);
                    }
                }

                return Some((batch, param));
            }

            let next = batch.next.load(Ordering::Acquire);
            if next.is_null() {
                return None;
            }

            // try to advance head, but continue regardless
            match self.head.compare_exchange_weak(
                current,
                next,
                Ordering::Release,
                Ordering::Acquire,
            ) {
                Ok(_) => {
                    self.release_viewer(current);
                    current = next;
                }
                Err(new_head) => {
                    current = new_head;
                }
            }
        }
    }

    pub fn release_viewer(&self, batch_ptr: *mut TaskBatch) {
        if batch_ptr.is_null() {
            return;
        }

        let is_zero = unsafe { (&*batch_ptr).viewers_decrement_and_is_zero() };
        if is_zero {
            self.drop_from_oldest(batch_ptr);
        }
    }

    pub fn drop_from_oldest(&self, current: *mut TaskBatch) {
        let oldest = self.oldest.load(Ordering::Acquire);
        if oldest.is_null() || current != oldest {
            return;
        }

        let mut node = oldest;
        while !node.is_null() {
            let viewers = unsafe { (&*node).viewers_count() };
            if viewers != 0 {
                break;
            }

            let next = unsafe { (&*node).next.load(Ordering::Acquire) };
            unsafe {
                drop(Box::from_raw(node));
            }
            node = next;
        }

        if node != oldest {
            self.oldest.store(node, Ordering::Release);
        }
    }

    pub fn notify(&self, mut count: usize) {
        let num_workers = self.threads.len();
        count = count.min(num_workers);

        // the added contention of a 'start_from' shared atomic tends to be slower
        // than iterating over the padded atomics array, even if its from the start every time
        for i in 0..num_workers {
            if self.threads_is_awake[i]
                .compare_exchange(false, true, Ordering::SeqCst, Ordering::Relaxed)
                .is_ok()
            {
                unsafe {
                    (*self.threads[i].get()).assume_init_ref().unpark();
                    if count == 1 {
                        break;
                    }
                    count -= 1;
                }
            }
        }
    }

    pub fn register_worker_thread(&self, worker_id: usize) {
        unsafe {
            (*self.threads[worker_id].get()).write(thread::current());
        }
    }

    // wait until work is available or shutdown
    // returns true if work is available, false if shutdown
    pub fn wait_for_work(&self, worker_id: usize) -> bool {
        let mut is_awake = true;

        loop {
            if self.has_tasks() {
                return true;
            }
            if self.is_shutdown() {
                return false;
            }

            if is_awake {
                is_awake = false;
                self.threads_is_awake[worker_id].store(false, Ordering::SeqCst);

                if self.has_tasks() {
                    self.threads_is_awake[worker_id].store(true, Ordering::SeqCst);
                    return true;
                }
            }

            thread::park();
        }
    }

    pub fn is_shutdown(&self) -> bool {
        self.shutdown.load(Ordering::Acquire)
    }

    pub fn has_tasks(&self) -> bool {
        let tail = self.tail.load(Ordering::SeqCst);
        unsafe { (&*tail).has_unclaimed_tasks() }
    }

    pub fn shutdown(&self) {
        self.shutdown.store(true, Ordering::Release);

        self.threads.iter().for_each(|t| unsafe {
            (*t.get()).assume_init_ref().unpark();
        });
    }
}

impl Drop for Queue {
    fn drop(&mut self) {
        for thread in self.threads.iter() {
            unsafe {
                (*thread.get()).assume_init_drop();
            }
        }

        let mut current = self.oldest.load(Ordering::Acquire);
        while !current.is_null() {
            let next = unsafe { (&*current).next.load(Ordering::Acquire) };
            unsafe {
                drop(Box::from_raw(current));
            }
            current = next;
        }
    }
}
