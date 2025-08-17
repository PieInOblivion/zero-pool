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
    /// Creates a new thread pool with worker count equal to available parallelism
    ///
    /// Worker count is determined by `std::thread::available_parallelism()`,
    /// falling back to 1 if unavailable. This is usually the optimal choice
    /// for CPU-bound workloads.
    ///
    /// # Examples
    ///
    /// ```rust
    /// let pool = ZeroPool::new(); // Creates pool with optimal worker count
    /// ```
    pub fn new() -> Self {
        let worker_count = std::thread::available_parallelism()
            .map(std::num::NonZeroUsize::get)
            .unwrap_or(1);
        Self::with_workers(worker_count)
    }

    /// Creates a new thread pool with the specified number of workers
    ///
    /// Use this when you need precise control over the worker count,
    /// for example when coordinating with other thread pools or
    /// when you know the optimal count for your specific workload.
    ///
    /// # Panics
    ///
    /// Panics if `worker_count` is 0.
    ///
    /// # Examples
    ///
    /// ```rust
    /// let pool = ZeroPool::with_workers(4); // Exactly 4 workers
    /// ```
    pub fn with_workers(worker_count: usize) -> Self {
        assert!(worker_count > 0, "Must have at least one worker");

        let queue = Arc::new(BatchQueue::new());

        let workers: Vec<JoinHandle<()>> = (0..worker_count)
            .map(|id| spawn_worker(id, queue.clone()))
            .collect();

        ZeroPool { queue, workers }
    }

    /// Submit a single task using raw function and parameter pointers
    ///
    /// This is the lowest-level submission method with minimal overhead.
    /// Most users should prefer the type-safe `submit_task` method.
    ///
    /// # Safety
    ///
    /// The parameter pointer must remain valid until the returned future completes.
    /// The caller must ensure the pointer points to the correct parameter type
    /// expected by the task function.
    ///
    /// # Examples
    ///
    /// ```rust
    /// let params = MyTask::new(42, &mut result);
    /// let future = pool.submit_raw_task(
    ///     my_task as TaskFnPointer,
    ///     &params as *const _ as TaskParamPointer
    /// );
    /// future.wait();
    /// ```
    pub fn submit_raw_task(&self, task_fn: TaskFnPointer, params: TaskParamPointer) -> WorkFuture {
        self.queue.push_single_task(task_fn, params)
    }

    /// Submit a batch of tasks using raw work items
    ///
    /// This method provides optimal performance for pre-converted task batches.
    /// Use `uniform_tasks_to_pointers` to convert typed tasks efficiently,
    /// or build work items manually for mixed task types.
    ///
    /// # Examples
    ///
    /// ```rust
    /// let tasks = uniform_tasks_to_pointers(my_task, &params_vec);
    /// let future = pool.submit_raw_task_batch(&tasks);
    /// future.wait();
    /// ```
    pub fn submit_raw_task_batch(&self, tasks: &[WorkItem]) -> WorkFuture {
        self.queue.push_task_batch(tasks)
    }

    /// Submit a single typed task with automatic pointer conversion
    ///
    /// This method provides type safety while maintaining performance.
    /// The parameter struct must remain valid until the future completes.
    /// This is the recommended method for submitting individual tasks.
    ///
    /// # Examples
    ///
    /// ```rust
    /// let task_params = MyTask::new(42, &mut result);
    /// let future = pool.submit_task(my_task_fn, &task_params);
    /// future.wait();
    /// assert_eq!(result, expected_value);
    /// ```
    pub fn submit_task<T>(&self, task_fn: TaskFnPointer, params: &T) -> WorkFuture {
        let params_ptr = params as *const T as TaskParamPointer;
        self.submit_raw_task(task_fn, params_ptr)
    }

    /// Submit a batch of uniform tasks with automatic pointer conversion
    ///
    /// All tasks in the batch must be the same type and use the same task function.
    /// This method handles the pointer conversion automatically and is the most
    /// convenient way to submit large batches of similar work.
    ///
    /// # Examples
    ///
    /// ```rust
    /// let tasks: Vec<_> = (0..1000)
    ///     .map(|i| MyTask::new(i, &mut results[i]))
    ///     .collect();
    /// let future = pool.submit_batch_uniform(my_task_fn, &tasks);
    /// future.wait();
    /// ```
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
