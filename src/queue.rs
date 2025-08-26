use crate::padded_type::PaddedAtomicPtr;
use crate::task_batch::TaskBatch;
use crate::wait::SleepNotifier;
use crate::{TaskFnPointer, task_future::TaskFuture};
use crate::{TaskItem, TaskParamPointer};
use std::sync::atomic::{AtomicBool, Ordering};

pub struct Queue {
    head: PaddedAtomicPtr<TaskBatch>,
    tail: PaddedAtomicPtr<TaskBatch>,
    notifier: SleepNotifier,
    shutdown: AtomicBool,
}

impl Queue {
    pub fn new(worker_count: usize) -> Self {
        let anchor_node =
            Box::into_raw(Box::new(TaskBatch::new(Box::from([]), TaskFuture::new(0))));
        Queue {
            head: PaddedAtomicPtr::new(anchor_node),
            tail: PaddedAtomicPtr::new(anchor_node),
            notifier: SleepNotifier::new(worker_count),
            shutdown: AtomicBool::new(false),
        }
    }

    pub fn push_batch(&self, items: Box<[TaskItem]>, future: TaskFuture) {
        let new_batch = Box::into_raw(Box::new(TaskBatch::new(items, future)));

        let prev_tail = self.tail.swap(new_batch, Ordering::AcqRel);

        unsafe {
            (*prev_tail).next.store(new_batch, Ordering::Release);
        }
        self.notifier.fast_notify();
    }

    pub fn get_next_batch(&self) -> Option<(*mut TaskBatch, TaskItem, &TaskFuture)> {
        let mut current = self.head.load(Ordering::Acquire);

        loop {
            let batch = unsafe { &*current };

            if let Some(task) = batch.claim_next_item() {
                return Some((current, task, &batch.future));
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

    // bool returns if shutdown has been set
    pub fn wait_for_signal(&self) -> bool {
        self.notifier.wait_for(|| self.has_tasks(), &self.shutdown)
    }

    pub fn has_tasks(&self) -> bool {
        let tail = self.tail.load(Ordering::Acquire);
        unsafe { (&*tail).has_unclaimed_tasks() }
    }

    pub fn shutdown(&self) {
        self.shutdown.store(true, Ordering::Relaxed);
        self.notifier.safe_notify();
    }

    pub fn push_single_task(&self, task_fn: TaskFnPointer, params: TaskParamPointer) -> TaskFuture {
        let future = TaskFuture::new(1);
        self.push_batch(Box::from([(task_fn, params)]), future.clone());
        future
    }

    pub fn push_task_batch(&self, tasks: &[TaskItem]) -> TaskFuture {
        if tasks.is_empty() {
            return TaskFuture::new(0);
        }

        let future = TaskFuture::new(tasks.len());

        self.push_batch(Box::from(tasks), future.clone());
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
