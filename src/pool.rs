use crate::{
    TaskFnPointer, TaskParamPointer, future::WorkFuture, queue::BatchQueue,
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

    pub fn submit_raw_task(&self, params: TaskParamPointer, task_fn: TaskFnPointer) -> WorkFuture {
        self.queue.push_single_task(params, task_fn)
    }

    pub fn submit_raw_task_batch(
        &self,
        tasks: &Vec<(TaskParamPointer, TaskFnPointer)>,
    ) -> WorkFuture {
        self.queue.push_task_batch(tasks)
    }

    pub fn submit_task<T>(&self, params: &T, task_fn: TaskFnPointer) -> WorkFuture {
        let params_ptr = params as *const T as TaskParamPointer;
        self.submit_raw_task(params_ptr, task_fn)
    }

    pub fn submit_batch_uniform<T>(&self, params_vec: &[T], task_fn: TaskFnPointer) -> WorkFuture {
        let tasks = uniform_tasks_to_pointers(params_vec, task_fn);
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
