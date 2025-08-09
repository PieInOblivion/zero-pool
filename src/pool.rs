use crate::{
    TaskFnPointer, TaskParamPointer, WorkItem, future::WorkFuture, queue::BatchQueue,
    uniform_tasks_to_pointers, worker::Worker,
};
use std::sync::Arc;

pub struct ThreadPool {
    workers: Vec<Worker>,
    queue: Arc<BatchQueue>,
}

impl ThreadPool {
    pub fn new(worker_count: usize) -> Self {
        assert!(worker_count > 0, "Must have at least one worker");

        let queue = Arc::new(BatchQueue::new());

        let workers: Vec<Worker> = (0..worker_count)
            .map(|id| Worker::new(id, queue.clone()))
            .collect();

        ThreadPool { workers, queue }
    }

    pub fn submit_raw_task(&self, task_fn: TaskFnPointer, params: TaskParamPointer) -> WorkFuture {
        self.queue.push_single_task(task_fn, params)
    }

    pub fn submit_raw_task_batch(&self, tasks: &[WorkItem]) -> WorkFuture {
        self.queue.push_task_batch(tasks)
    }

    pub fn submit_task<T>(&self, task_fn: TaskFnPointer, params: &T) -> WorkFuture {
        let params_ptr = params as *const T as TaskParamPointer;
        self.submit_raw_task(task_fn, params_ptr)
    }

    pub fn submit_batch_uniform<T>(&self, task_fn: TaskFnPointer, params_vec: &[T]) -> WorkFuture {
        let tasks = uniform_tasks_to_pointers(task_fn, params_vec);
        self.submit_raw_task_batch(&tasks)
    }

    pub fn worker_count(&self) -> usize {
        self.workers.len()
    }

    pub fn total_pending(&self) -> usize {
        self.queue.len()
    }
}

impl Drop for ThreadPool {
    fn drop(&mut self) {
        self.queue.shutdown();

        let workers = std::mem::take(&mut self.workers);
        for worker in workers {
            let _ = worker.handle.join();
        }
    }
}
