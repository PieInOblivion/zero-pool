use crate::{TaskFn, future::WorkFuture, work_item::WorkItem};
use std::ptr;
use std::sync::atomic::{AtomicBool, AtomicPtr, AtomicUsize, Ordering};
use std::sync::{Condvar, Mutex};

pub struct WorkBatch {
    pub items: Vec<WorkItem>,
    pub future: WorkFuture,
    next_item: AtomicUsize,
    next: AtomicPtr<WorkBatch>,
}

impl WorkBatch {
    pub fn new(items: Vec<WorkItem>, future: WorkFuture) -> Self {
        WorkBatch {
            items,
            future,
            next_item: AtomicUsize::new(0),
            next: AtomicPtr::new(ptr::null_mut()),
        }
    }

    pub fn claim_next_item(&self) -> Option<&WorkItem> {
        let item_index = self.next_item.fetch_add(1, Ordering::Relaxed);

        if item_index < self.items.len() {
            Some(&self.items[item_index])
        } else {
            None
        }
    }

    pub fn has_work(&self) -> bool {
        self.next_item.load(Ordering::Relaxed) < self.items.len()
    }

    pub fn remaining_items(&self) -> usize {
        let claimed = self.next_item.load(Ordering::Relaxed);
        self.items.len().saturating_sub(claimed)
    }
}

unsafe impl Send for BatchQueue {}
unsafe impl Sync for BatchQueue {}

pub struct BatchQueue {
    head: AtomicPtr<WorkBatch>,
    tail: AtomicPtr<WorkBatch>,

    shutdown: AtomicBool,

    condvar_mutex: Mutex<()>,
    condvar: Condvar,
}

impl BatchQueue {
    pub fn new() -> Self {
        let dummy = Box::into_raw(Box::new(WorkBatch::new(Vec::new(), WorkFuture::new(0))));

        BatchQueue {
            head: AtomicPtr::new(dummy),
            tail: AtomicPtr::new(dummy),
            shutdown: AtomicBool::new(false),
            condvar_mutex: Mutex::new(()),
            condvar: Condvar::new(),
        }
    }

    pub fn push_batch(&self, items: Vec<WorkItem>, future: WorkFuture) {
        if items.is_empty() {
            return;
        }

        let new_batch = Box::into_raw(Box::new(WorkBatch::new(items, future)));

        let prev_tail = self.tail.swap(new_batch, Ordering::AcqRel);

        unsafe {
            (*prev_tail).next.store(new_batch, Ordering::Release);
        }

        // NOTE: Notify all and having one empty spin per worker
        // proved to have better performance than waking x workers for x tasks
        self.condvar.notify_all();
    }

    pub fn claim_work(&self) -> Option<(&WorkItem, &WorkFuture)> {
        let mut current = self.head.load(Ordering::Acquire);

        while !current.is_null() {
            let batch = unsafe { &*current };

            if let Some(work_item) = batch.claim_next_item() {
                return Some((work_item, &batch.future));
            }

            let next = batch.next.load(Ordering::Acquire);
            if !next.is_null() {
                let _ = self.head.compare_exchange_weak(
                    current,
                    next,
                    Ordering::AcqRel,
                    Ordering::Relaxed,
                );
                current = next;
            } else {
                break;
            }
        }

        None
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

    pub fn len(&self) -> usize {
        let mut total = 0;
        let mut current = self.head.load(Ordering::Acquire);

        while !current.is_null() {
            let batch = unsafe { &*current };
            total += batch.remaining_items();
            current = batch.next.load(Ordering::Acquire);
        }

        total
    }

    pub fn shutdown(&self) {
        self.shutdown.store(true, Ordering::Release);
        self.condvar.notify_all();
    }

    pub fn is_shutdown(&self) -> bool {
        self.shutdown.load(Ordering::Acquire)
    }

    pub fn push_single_task(&self, params: *const (), task_fn: TaskFn) -> WorkFuture {
        let future = WorkFuture::new(1);
        let work_item = WorkItem::new(params, task_fn);
        self.push_batch(vec![work_item], future.clone());
        future
    }

    pub fn push_task_batch(&self, tasks: Vec<(*const (), TaskFn)>) -> WorkFuture {
        if tasks.is_empty() {
            return WorkFuture::new(0);
        }

        let future = WorkFuture::new(tasks.len());

        let work_items: Vec<WorkItem> = tasks
            .into_iter()
            .map(|(params, task_fn)| WorkItem::new(params, task_fn))
            .collect();

        self.push_batch(work_items, future.clone());
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
