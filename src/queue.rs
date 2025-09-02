use crate::TaskParamPointer;
use crate::padded_type::{PaddedAtomicBool, PaddedAtomicPtr};
use crate::task_batch::TaskBatch;
use crate::{TaskFnPointer, task_future::TaskFuture};
use std::cell::UnsafeCell;
use std::sync::atomic::{AtomicBool, Ordering};
use std::thread::{self, Thread};

pub struct Queue {
    head: PaddedAtomicPtr<TaskBatch>,
    tail: PaddedAtomicPtr<TaskBatch>,
    parked: Box<[PaddedAtomicBool]>,
    threads: Box<[UnsafeCell<Option<Thread>>]>,
    shutdown: AtomicBool,
}

unsafe impl Sync for Queue {}

impl Queue {
    pub fn new(worker_count: usize) -> Self {
        fn noop(_: *const ()) {}
        let anchor_node = Box::into_raw(Box::new(TaskBatch::new::<u8>(
            noop,
            &[],
            TaskFuture::new(0),
        )));

        let mut p = Vec::with_capacity(worker_count);
        let mut t = Vec::with_capacity(worker_count);

        for _ in 0..worker_count {
            p.push(PaddedAtomicBool::new(false));
            t.push(UnsafeCell::new(None));
        }

        Queue {
            head: PaddedAtomicPtr::new(anchor_node),
            tail: PaddedAtomicPtr::new(anchor_node),
            parked: p.into_boxed_slice(),
            threads: t.into_boxed_slice(),
            shutdown: AtomicBool::new(false),
        }
    }

    pub fn push_task_batch<T>(&self, task_fn: TaskFnPointer, params: &[T]) -> TaskFuture {
        if params.is_empty() {
            return TaskFuture::new(0);
        }

        let future = TaskFuture::new(params.len());
        let new_batch = Box::into_raw(Box::new(TaskBatch::new(task_fn, params, future.clone())));

        let prev_tail = self.tail.swap(new_batch, Ordering::AcqRel);
        unsafe {
            (*prev_tail).next.store(new_batch, Ordering::Release);
        }

        self.notify(params.len());
        future
    }

    pub fn get_next_batch(&self) -> Option<(&TaskBatch, TaskParamPointer, &TaskFuture)> {
        let mut current = self.head.load(Ordering::Acquire);

        loop {
            let batch = unsafe { &*current };

            if let Some(param) = batch.claim_next_param() {
                return Some((batch, param, &batch.future));
            }

            let next = batch.next.load(Ordering::Acquire);
            if next.is_null() {
                return None;
            }

            // help advance head past empty batch
            let _ = self.head.compare_exchange_weak(
                current,
                next,
                Ordering::Release,
                Ordering::Relaxed,
            );
            current = next;
        }
    }

    pub fn notify(&self, mut count: usize) {
        count = count.min(self.threads.len());

        for i in 0..self.threads.len() {
            if self.parked[i].load(Ordering::Acquire) {
                unsafe {
                    if let Some(t) = (&*self.threads[i].get()).as_ref() {
                        t.unpark();
                    }
                }
                count -= 1;
            }

            if count == 0 {
                break;
            }
        }
    }

    pub fn register_worker_thread(&self, worker_id: usize) {
        unsafe {
            *self.threads[worker_id].get() = Some(thread::current());
        }
    }

    // wait until work is available or shutdown
    pub fn wait_for_signal(&self, worker_id: usize) {
        self.parked[worker_id].store(true, Ordering::Release);

        while !self.has_tasks() && !self.is_shutdown() {
            thread::park();
        }

        self.parked[worker_id].store(false, Ordering::Release);
    }

    pub fn is_shutdown(&self) -> bool {
        self.shutdown.load(Ordering::Acquire)
    }

    pub fn has_tasks(&self) -> bool {
        let tail = self.tail.load(Ordering::Acquire);
        unsafe { (&*tail).has_unclaimed_tasks() }
    }

    pub fn shutdown(&self) {
        self.shutdown.store(true, Ordering::Release);
        self.notify(usize::MAX);
    }
}

impl Drop for Queue {
    fn drop(&mut self) {
        let mut current = self.head.load(Ordering::Relaxed);
        while !current.is_null() {
            let batch = unsafe { Box::from_raw(current) };
            current = batch.next.load(Ordering::Relaxed);
        }
    }
}
