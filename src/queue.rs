use crate::padded_type::PaddedAtomicPtr;
use crate::task_batch::TaskBatch;
use crate::wait_gate::WaitGate;
use crate::{TaskFnPointer, task_future::TaskFuture};
use crate::{TaskItem, TaskParamPointer};
use std::sync::atomic::{AtomicBool, Ordering};

pub struct Queue {
    head: PaddedAtomicPtr<TaskBatch>,
    tail: PaddedAtomicPtr<TaskBatch>,

    shutdown: AtomicBool,

    gate: WaitGate,
}

impl Queue {
    pub fn new() -> Self {
        let anchor_node = Box::into_raw(Box::new(TaskBatch::new(Vec::new(), TaskFuture::new(0))));

        Queue {
            head: PaddedAtomicPtr::new(anchor_node),
            tail: PaddedAtomicPtr::new(anchor_node),
            shutdown: AtomicBool::new(false),
            gate: WaitGate::new(),
        }
    }

    pub fn push_batch(&self, items: Vec<TaskItem>, future: TaskFuture) {
        let new_batch = Box::into_raw(Box::new(TaskBatch::new(items, future)));

        // Lock-free link (two atomics)
        let prev_tail = self.tail.swap(new_batch, Ordering::AcqRel);
        unsafe {
            (*prev_tail).next.store(new_batch, Ordering::Release);
        }

        // Only notify if sleepers
        self.gate.notify_all_if_waiters();
    }

    pub fn claim_task(&self) -> Option<(TaskItem, &TaskFuture)> {
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

            // try to advance head, helps other threads skip this empty batche
            let _ = self.head.compare_exchange_weak(
                current,
                next,
                Ordering::Release,
                Ordering::Relaxed,
            );
            current = next;
        }
    }

    // bool returns if shutdown has been set
    pub fn wait_for_signal(&self) {
        self.gate
            .wait_until(|| self.has_tasks() || self.shutdown.load(Ordering::Relaxed));
    }

    pub fn has_tasks(&self) -> bool {
        let tail = self.tail.load(Ordering::Acquire);
        unsafe { (&*tail).has_unclaimed_tasks() }
    }

    pub fn is_shutdown(&self) -> bool {
        self.shutdown.load(Ordering::Relaxed)
    }

    pub fn shutdown(&self) {
        self.shutdown.store(true, Ordering::Relaxed);
        self.gate.notify_all();
    }

    pub fn push_single_task(&self, task_fn: TaskFnPointer, params: TaskParamPointer) -> TaskFuture {
        let future = TaskFuture::new(1);
        self.push_batch(vec![(task_fn, params)], future.clone());
        future
    }

    pub fn push_task_batch(&self, tasks: &[TaskItem]) -> TaskFuture {
        if tasks.is_empty() {
            return TaskFuture::new(0);
        }

        let future = TaskFuture::new(tasks.len());

        self.push_batch(tasks.to_owned(), future.clone());
        future
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
