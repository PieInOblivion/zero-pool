use crate::{
    TaskFnPointer, TaskParamPointer, WorkItem, future::WorkFuture, queue::BatchQueue,
    uniform_tasks_to_pointers, worker::spawn_worker,
};
use std::{sync::Arc, thread::JoinHandle};

pub struct ZeroPool {
    queue: Arc<BatchQueue>,
    workers: Vec<JoinHandle<()>>,
}

impl ZeroPool {
    pub fn new() -> Self {
        let worker_count = std::thread::available_parallelism()
            .map(std::num::NonZeroUsize::get)
            .unwrap_or(1);
        Self::with_workers(worker_count)
    }

    pub fn with_workers(worker_count: usize) -> Self {
        assert!(worker_count > 0, "Must have at least one worker");

        let queue = Arc::new(BatchQueue::new());

        let workers: Vec<JoinHandle<()>> = (0..worker_count)
            .map(|id| spawn_worker(id, queue.clone()))
            .collect();

        ZeroPool { queue, workers }
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
}

impl Drop for ZeroPool {
    fn drop(&mut self) {
        self.queue.shutdown();

        let workers = std::mem::take(&mut self.workers);
        for handle in workers {
            let _ = handle.join();
        }
    }
}
