use crate::padded_type::PaddedType;
use crate::task_batch::TaskBatch;
use crate::{TaskFnPointer, TaskFuture, TaskParamPointer};
use std::cell::UnsafeCell;
use std::mem::MaybeUninit;
use std::sync::atomic::{AtomicBool, AtomicPtr, AtomicU8, AtomicUsize, Ordering};
use std::thread::{self, Thread};

pub const NOT_IN_CRITICAL: usize = usize::MAX;
pub const EPOCH_MASK: usize = usize::MAX >> 1; // use only lower bits for epoch

pub struct Queue {
    head: PaddedType<AtomicPtr<TaskBatch>>,
    tail: PaddedType<AtomicPtr<TaskBatch>>,
    epoch_ticker: PaddedType<AtomicU8>,
    global_epoch: PaddedType<AtomicUsize>,
    local_epochs: Box<[PaddedType<AtomicUsize>]>,
    threads: Box<[UnsafeCell<MaybeUninit<Thread>>]>,
    shutdown: AtomicBool,
}

// needed for 'threads'
unsafe impl Sync for Queue {}

impl Queue {
    pub fn new(worker_count: usize) -> Self {
        fn noop(_: TaskParamPointer) {}
        let anchor_node = Box::into_raw(Box::new(TaskBatch::new::<u8>(
            noop,
            &[],
            TaskFuture::new(0),
        )));

        let local_epochs: Box<[_]> = (0..worker_count)
            .map(|_| PaddedType::new(AtomicUsize::new(NOT_IN_CRITICAL)))
            .collect();

        let threads: Box<[_]> = (0..worker_count)
            .map(|_| UnsafeCell::new(MaybeUninit::uninit()))
            .collect();

        Queue {
            head: PaddedType::new(AtomicPtr::new(anchor_node)),
            tail: PaddedType::new(AtomicPtr::new(anchor_node)),
            global_epoch: PaddedType::new(AtomicUsize::new(0)),
            epoch_ticker: PaddedType::new(AtomicU8::new(0)),
            local_epochs,
            threads,
            shutdown: AtomicBool::new(false),
        }
    }

    pub fn push_task_batch<T>(&self, task_fn: fn(&T), params: &[T]) -> TaskFuture {
        if params.is_empty() {
            return TaskFuture::new(0);
        }

        let future = TaskFuture::new(params.len());

        let raw_fn: TaskFnPointer = unsafe { std::mem::transmute(task_fn) };
        let new_batch = Box::into_raw(Box::new(TaskBatch::new(raw_fn, params, future.clone())));

        let prev_tail = self.tail.swap(new_batch, Ordering::SeqCst);
        unsafe {
            (*prev_tail).next.store(new_batch, Ordering::Release);
        }

        self.notify(params.len());
        future
    }

    pub fn get_next_batch(
        &self,
        worker_id: usize,
        cached_local_epoch: &mut usize,
        garbage_head: &mut *mut TaskBatch,
        garbage_tail: &mut *mut TaskBatch,
    ) -> Option<(&TaskBatch, TaskParamPointer)> {
        let global_epoch = self.global_epoch.load(Ordering::Relaxed) & EPOCH_MASK;
        // if our epoch is already current then avoid the SeqCst barrier
        if *cached_local_epoch != global_epoch {
            *cached_local_epoch = global_epoch;
            // SeqCst acts as a full barrier to publish epoch before touching queue nodes,
            // preventing reclamation races on weak memory models
            self.local_epochs[worker_id].store(global_epoch, Ordering::SeqCst);
        }

        let mut current = self.head.load(Ordering::Acquire);

        loop {
            let batch = unsafe { &*current };

            if let Some(param) = batch.claim_next_param() {
                return Some((batch, param));
            }

            let next = batch.next.load(Ordering::Acquire);
            if next.is_null() {
                return None;
            }

            match self.head.compare_exchange_weak(
                current,
                next,
                Ordering::Release,
                Ordering::Acquire,
            ) {
                Ok(_) => {
                    // 1. SAFETY PATCH: Fetch a fresh epoch AFTER/DURING unlinking to prevent preemption use-after-free
                    let fresh_epoch = self.global_epoch.load(Ordering::Relaxed) & EPOCH_MASK;

                    unsafe {
                        // 2. Stamp with fresh epoch (plain write now)
                        (*current).epoch = fresh_epoch;

                        // 3. Intrusive link to worker's local garbage chain
                        (*current).local_garbage_next = std::ptr::null_mut();
                        if garbage_head.is_null() {
                            *garbage_head = current;
                        } else {
                            (**garbage_tail).local_garbage_next = current;
                        }
                        *garbage_tail = current;
                    }
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
            if self.local_epochs[i].load(Ordering::SeqCst) == NOT_IN_CRITICAL {
                unsafe {
                    (*self.threads[i].get()).assume_init_ref().unpark();
                    count -= 1;
                    if count == 0 {
                        break;
                    }
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
    pub fn wait_for_work(&self, worker_id: usize, cached_local_epoch: &mut usize) -> bool {
        loop {
            if self.has_tasks() {
                return true;
            }
            if self.is_shutdown() {
                return false;
            }

            if *cached_local_epoch != NOT_IN_CRITICAL {
                *cached_local_epoch = NOT_IN_CRITICAL;
                self.local_epochs[worker_id].store(NOT_IN_CRITICAL, Ordering::SeqCst);

                if self.has_tasks() {
                    return true;
                }
            }

            thread::park();
        }
    }

    pub fn min_active_epoch(&self) -> usize {
        let mut min_epoch = self.global_epoch.load(Ordering::Relaxed) & EPOCH_MASK;
        for local_epoch in self.local_epochs.iter() {
            let e = local_epoch.load(Ordering::SeqCst);
            if e != NOT_IN_CRITICAL {
                // Handle wrapping correctly to find the oldest
                if min_epoch.wrapping_sub(e) & EPOCH_MASK < (EPOCH_MASK / 2) {
                    min_epoch = e;
                }
            }
        }
        min_epoch
    }

    pub fn batch_completed(&self) {
        if self.epoch_ticker.fetch_add(1, Ordering::Relaxed) == u8::MAX {
            self.global_epoch.fetch_add(1, Ordering::Relaxed);
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

        let mut current = self.head.load(Ordering::Relaxed);
        while !current.is_null() {
            let batch = unsafe { Box::from_raw(current) };
            current = batch.next.load(Ordering::Relaxed);
        }
    }
}
