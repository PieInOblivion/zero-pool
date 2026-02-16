use crate::padded_type::PaddedType;
use crate::task_batch::TaskBatch;
use crate::{TaskFnPointer, TaskFuture, TaskParamPointer};
use std::cell::UnsafeCell;
use std::mem::MaybeUninit;
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, AtomicPtr, Ordering};
use std::thread::{self, Thread};

pub struct Queue {
    head: PaddedType<AtomicPtr<TaskBatch>>,
    tail: PaddedType<AtomicPtr<TaskBatch>>,
    threads_is_awake: Box<[PaddedType<AtomicBool>]>,
    threads: Box<[UnsafeCell<MaybeUninit<Thread>>]>,
    shutdown: AtomicBool,
}

// needed for 'threads'
unsafe impl Sync for Queue {}

impl Queue {
    pub fn new(worker_count: usize) -> Self {
        fn noop(_: TaskParamPointer) {}
        let anchor_node = Box::into_raw(Box::new(TaskBatch::new::<u8>(noop, &[])));
        unsafe {
            (&*anchor_node).viewers_decrement();
        }

        let threads_is_awake: Box<[_]> = (0..worker_count)
            .map(|_| PaddedType::new(AtomicBool::new(false)))
            .collect();

        let threads: Box<[_]> = (0..worker_count)
            .map(|_| UnsafeCell::new(MaybeUninit::uninit()))
            .collect();

        Queue {
            head: PaddedType::new(AtomicPtr::new(anchor_node)),
            tail: PaddedType::new(AtomicPtr::new(anchor_node)),
            threads_is_awake,
            threads,
            shutdown: AtomicBool::new(false),
        }
    }

    pub fn register_worker_thread(&self, worker_id: usize) {
        unsafe {
            (*self.threads[worker_id].get()).write(thread::current());
        }
    }

    pub fn push_task_batch<T>(self: &Arc<Self>, task_fn: fn(&T), params: &[T]) -> TaskFuture {
        let raw_fn: TaskFnPointer = unsafe { std::mem::transmute(task_fn) };
        let new_batch = Box::into_raw(Box::new(TaskBatch::new(raw_fn, params)));

        let prev_tail = self.tail.swap(new_batch, Ordering::SeqCst);
        unsafe {
            (*prev_tail).next.store(new_batch, Ordering::Release);
        }
        println!("New Batch: {:?}", new_batch);
        self.notify(params.len());
        TaskFuture::from_batch(new_batch)
    }

    pub fn get_next_batch(
        &self,
        last_incremented_on: &mut *mut TaskBatch,
        retired_batches: &mut Vec<*mut TaskBatch>,
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
                        unsafe { (&*previous).viewers_decrement() };
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
                    retired_batches.push(current);
                    current = next;
                }
                Err(new_head) => {
                    current = new_head;
                }
            }
        }
    }

    pub fn notify(&self, mut count: usize) {
        let num_workers = self.threads.len();
        count = count.min(num_workers);

        // the added contention of a 'start_from' shared atomic tends to be slower
        // than iterating over the padded atomics array, even if its from the start every time
        for i in 0..num_workers {
            if count == 0 {
                break;
            }

            if self.threads_is_awake[i]
                .compare_exchange(false, true, Ordering::SeqCst, Ordering::Relaxed)
                .is_ok()
            {
                unsafe {
                    (*self.threads[i].get()).assume_init_ref().unpark();
                    count -= 1;
                }
            }
        }
    }

    // wait until work is available or shutdown
    // returns true if work is available, false if shutdown
    pub fn wait_for_work(&self, worker_id: usize) -> bool {
        loop {
            if self.has_tasks() {
                return true;
            }
            if self.is_shutdown() {
                return false;
            }

            self.threads_is_awake[worker_id].store(false, Ordering::SeqCst);

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

        let mut current = self.head.load(Ordering::Acquire);
        while !current.is_null() {
            let next = unsafe { (&*current).next.load(Ordering::Acquire) };
            unsafe {
                drop(Box::from_raw(current));
            }
            println!("Drop() Batch: {:?}", current);
            current = next;
        }
    }
}
