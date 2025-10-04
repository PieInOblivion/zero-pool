use crate::{TaskFnPointer, queue::Queue, task_future::TaskFuture, worker::spawn_worker};
use std::{
    sync::{Arc, Barrier},
    thread::JoinHandle,
};

pub struct ZeroPool {
    queue: Arc<Queue>,
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
    /// use zero_pool::ZeroPool;
    /// let pool = ZeroPool::new();
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
    /// ```rust
    /// use zero_pool::ZeroPool;
    /// let pool = ZeroPool::with_workers(4);
    /// ```
    pub fn with_workers(worker_count: usize) -> Self {
        assert!(worker_count > 0, "Must have at least one worker");

        let queue = Arc::new(Queue::new(worker_count));

        let barrier = Arc::new(Barrier::new(worker_count + 1));
        let workers: Vec<JoinHandle<()>> = (0..worker_count)
            .map(|id| spawn_worker(id, queue.clone(), barrier.clone()))
            .collect();

        barrier.wait();

        ZeroPool { queue, workers }
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
    /// use zero_pool::{ZeroPool, zp_define_task_fn, zp_write};
    ///
    /// struct MyTaskParams { value: u64, result: *mut u64 }
    ///
    /// zp_define_task_fn!(my_task_fn, MyTaskParams, |params| {
    ///     zp_write!(params.result, params.value * 2);
    /// });
    ///
    /// let pool = ZeroPool::new();
    /// let mut result = 0u64;
    /// let task_params = MyTaskParams { value: 42, result: &mut result };
    /// let future = pool.submit_task(my_task_fn, &task_params);
    /// future.wait();
    /// assert_eq!(result, 84);
    /// ```
    pub fn submit_task<T>(&self, task_fn: TaskFnPointer, params: &T) -> TaskFuture {
        let slice = std::slice::from_ref(params);
        self.queue.push_task_batch(task_fn, slice)
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
    /// use zero_pool::{ZeroPool, zp_define_task_fn, zp_write};
    ///
    /// struct MyTaskParams { value: u64, result: *mut u64 }
    ///
    /// zp_define_task_fn!(my_task_fn, MyTaskParams, |params| {
    ///     zp_write!(params.result, params.value * 2);
    /// });
    ///
    /// let pool = ZeroPool::new();
    /// let mut results = vec![0u64; 1000];
    /// let task_params: Vec<_> = (0..1000)
    ///     .map(|i| MyTaskParams { value: i as u64, result: &mut results[i] })
    ///     .collect();
    /// let future = pool.submit_batch_uniform(my_task_fn, &task_params);
    /// future.wait();
    /// assert_eq!(results[0], 0);
    /// assert_eq!(results[1], 2);
    /// assert_eq!(results[999], 1998);
    /// ```
    pub fn submit_batch_uniform<T>(&self, task_fn: TaskFnPointer, params_vec: &[T]) -> TaskFuture {
        self.queue.push_task_batch(task_fn, params_vec)
    }
}

impl Default for ZeroPool {
    /// Creates a new thread pool with default settings
    ///
    /// Equivalent to calling `ZeroPool::new()`. Worker count is determined by
    /// `std::thread::available_parallelism()`, falling back to 1 if unavailable.
    fn default() -> Self {
        Self::new()
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
