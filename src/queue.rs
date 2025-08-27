use crate::TaskParamPointer;
use crate::padded_type::PaddedAtomicPtr;
use crate::task_batch::TaskBatch;
use crate::waiter::Waiter;
use crate::{TaskFnPointer, task_future::TaskFuture};
use std::sync::atomic::{AtomicBool, Ordering};

pub struct Queue {
    head: PaddedAtomicPtr<TaskBatch>,
    tail: PaddedAtomicPtr<TaskBatch>,
    waiter: Waiter,
    shutdown: AtomicBool,
}

impl Queue {
    pub fn new() -> Self {
        fn noop(_: *const ()) {}
        let anchor_node = Box::into_raw(Box::new(TaskBatch::new(
            noop,
            Box::from([]),
            TaskFuture::new(0),
        )));

        Queue {
            head: PaddedAtomicPtr::new(anchor_node),
            tail: PaddedAtomicPtr::new(anchor_node),
            waiter: Waiter::new(),
            shutdown: AtomicBool::new(false),
        }
    }

    pub fn push_batch(
        &self,
        task_fn: TaskFnPointer,
        params: Box<[TaskParamPointer]>,
        future: TaskFuture,
    ) {
        let new_batch = Box::into_raw(Box::new(TaskBatch::new(task_fn, params, future)));

        let prev_tail = self.tail.swap(new_batch, Ordering::AcqRel);
        unsafe {
            (*prev_tail).next.store(new_batch, Ordering::Release);
        }

        self.waiter.notify();
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

    // bool returns if shutdown has been set
    pub fn wait_for_signal(&self) -> bool {
        self.waiter.wait_for(|| self.has_tasks(), &self.shutdown)
    }

    pub fn has_tasks(&self) -> bool {
        let tail = self.tail.load(Ordering::Acquire);
        unsafe { (&*tail).has_unclaimed_tasks() }
    }

    pub fn shutdown(&self) {
        self.shutdown.store(true, Ordering::Relaxed);
        self.waiter.notify();
    }

    pub fn push_single_task(&self, task_fn: TaskFnPointer, params: TaskParamPointer) -> TaskFuture {
        let future = TaskFuture::new(1);
        self.push_batch(task_fn, Box::from([params]), future.clone());
        future
    }

    pub fn push_task_batch(
        &self,
        task_fn: TaskFnPointer,
        params: &[TaskParamPointer],
    ) -> TaskFuture {
        if params.is_empty() {
            return TaskFuture::new(0);
        }

        let future = TaskFuture::new(params.len());

        self.push_batch(task_fn, Box::from(params), future.clone());
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
