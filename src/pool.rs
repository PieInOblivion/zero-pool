use crate::{queue::Queue, task_future::TaskFuture, worker::spawn_worker};
use std::{
    num::NonZeroUsize,
    sync::{Arc, Barrier},
    thread::{self, JoinHandle},
};

pub struct ZeroPool {
    queue: Arc<Queue>,
    workers: Box<[JoinHandle<()>]>,
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
        let worker_count = thread::available_parallelism().unwrap_or(NonZeroUsize::MIN);
        Self::with_workers(worker_count)
    }

    /// Creates a new thread pool with the specified number of workers
    ///
    /// Use this when you need precise control over the worker count,
    /// for example when coordinating with other thread pools or
    /// when you know the optimal count for your specific workload.
    ///
    /// # Examples
    ///
    /// ```rust
    /// use std::num::NonZeroUsize;
    /// use zero_pool::ZeroPool;
    /// let pool = ZeroPool::with_workers(NonZeroUsize::new(4).unwrap());
    /// ```
    pub fn with_workers(worker_count: NonZeroUsize) -> Self {
        let worker_count = worker_count.get();

        let queue = Arc::new(Queue::new(worker_count));

        let barrier = Arc::new(Barrier::new(worker_count + 1));
        let workers = (0..worker_count)
            .map(|id| spawn_worker(id, queue.clone(), barrier.clone()))
            .collect();

        barrier.wait();

        ZeroPool { queue, workers }
    }

    /// Submit a single typed task
    ///
    /// The parameter struct must remain valid until the future completes.
    /// This is the recommended method for submitting individual tasks.
    ///
    /// # Examples
    ///
    /// ```rust
    /// use zero_pool::ZeroPool;
    ///
    /// struct MyTaskParams { value: u64, result: *mut u64 }
    ///
    /// fn my_task_fn(params: &MyTaskParams) {
    ///     unsafe { *params.result = params.value * 2; }
    /// }
    ///
    /// let pool = ZeroPool::new();
    /// let mut result = 0u64;
    /// let task_params = MyTaskParams { value: 42, result: &mut result };
    /// let future = pool.submit_task(my_task_fn, &task_params);
    /// future.wait();
    /// assert_eq!(result, 84);
    /// ```
    #[inline]
    pub fn submit_task<T>(&self, task_fn: fn(&T), params: &T) -> TaskFuture {
        let slice = std::slice::from_ref(params);
        self.queue.push_task_batch(task_fn, slice)
    }

    /// Submit a batch of uniform tasks
    ///
    /// All tasks in the batch must be the same type and use the same task function.
    /// This method handles the pointer conversion automatically and is the most
    /// convenient way to submit large batches of similar work.
    ///
    /// # Examples
    ///
    /// ```rust
    /// use zero_pool::ZeroPool;
    ///
    /// struct MyTaskParams { value: u64, result: *mut u64 }
    ///
    /// fn my_task_fn(params: &MyTaskParams) {
    ///     unsafe { *params.result = params.value * 2; }
    /// }
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
    #[inline]
    pub fn submit_batch_uniform<T>(&self, task_fn: fn(&T), params_vec: &[T]) -> TaskFuture {
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
