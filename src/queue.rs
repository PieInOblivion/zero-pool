use crate::padded_type::PaddedAtomicPtr;
use crate::work_batch::WorkBatch;
use crate::{TaskFnPointer, future::WorkFuture};
use crate::{TaskParamPointer, WorkItem};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Condvar, Mutex};

unsafe impl Send for BatchQueue {}
unsafe impl Sync for BatchQueue {}

pub struct BatchQueue {
    head: PaddedAtomicPtr<WorkBatch>,
    tail: PaddedAtomicPtr<WorkBatch>,

    shutdown: AtomicBool,

    condvar_mutex: Mutex<()>,
    condvar: Condvar,
}

impl BatchQueue {
    pub fn new() -> Self {
        let dummy = Box::into_raw(Box::new(WorkBatch::new(Vec::new(), WorkFuture::new(0))));

        BatchQueue {
            head: PaddedAtomicPtr::new(dummy),
            tail: PaddedAtomicPtr::new(dummy),
            shutdown: AtomicBool::new(false),
            condvar_mutex: Mutex::new(()),
            condvar: Condvar::new(),
        }
    }

    pub fn push_batch(&self, items: Vec<WorkItem>, future: WorkFuture) {
        let new_batch = Box::into_raw(Box::new(WorkBatch::new(items, future)));

        let prev_tail = self.tail.swap(new_batch, Ordering::AcqRel);

        unsafe {
            (*prev_tail).next.store(new_batch, Ordering::Release);
        }

        // NOTE: Notify all and having one empty spin per worker
        // proved to have better performance than waking x workers for x tasks
        self.condvar.notify_all();
    }

    pub fn claim_work(&self) -> Option<(WorkItem, &WorkFuture)> {
        let mut current = self.head.load(Ordering::Acquire);

        loop {
            let batch = unsafe { &*current };
            
            if let Some(work_item) = batch.claim_next_item() {
                return Some((work_item, &batch.future));
            }

            let next = batch.next.load(Ordering::Acquire);
            if next.is_null() {
                return None;
            }

            // try to advance head, helps other threads skip empty batches
            let _ = self.head.compare_exchange_weak(
                current,
                next,
                Ordering::Relaxed,
                Ordering::Relaxed,
            );
            current = next;
        }
    }

    pub fn wait_for_work(&self) {
        let mut guard = self.condvar_mutex.lock().unwrap();

        // check condition while holding the mutex to avoid race conditions
        while !self.has_work() && !self.is_shutdown() {
            guard = self.condvar.wait(guard).unwrap();
        }
    }

    pub fn has_work(&self) -> bool {
        let mut current = self.head.load(Ordering::Acquire);

        while !current.is_null() {
            let batch = unsafe { &*current };

            if batch.has_work() {
                return true;
            }

            current = batch.next.load(Ordering::Acquire);
        }

        false
    }

    pub fn shutdown(&self) {
        self.shutdown.store(true, Ordering::Release);

        let _guard = self.condvar_mutex.lock().unwrap();
        self.condvar.notify_all();
    }

    pub fn is_shutdown(&self) -> bool {
        self.shutdown.load(Ordering::Acquire)
    }

    pub fn push_single_task(&self, task_fn: TaskFnPointer, params: TaskParamPointer) -> WorkFuture {
        let future = WorkFuture::new(1);
        self.push_batch(vec![(task_fn, params)], future.clone());
        future
    }

    pub fn push_task_batch(&self, tasks: &[WorkItem]) -> WorkFuture {
        if tasks.is_empty() {
            return WorkFuture::new(0);
        }

        let future = WorkFuture::new(tasks.len());

        self.push_batch(tasks.to_owned(), future.clone());
        future
    }
}

impl Drop for BatchQueue {
    fn drop(&mut self) {
        let mut current = self.head.load(Ordering::Relaxed);
        while !current.is_null() {
            let batch = unsafe { Box::from_raw(current) };
            current = batch.next.load(Ordering::Relaxed);
        }
    }
}
