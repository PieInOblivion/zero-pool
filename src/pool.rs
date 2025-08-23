use crate::{
    TaskFnPointer, TaskItem, TaskParamPointer, queue::Queue, task_future::TaskFuture,
    uniform_tasks_to_pointers, worker::spawn_worker,
};
use std::{sync::Arc, thread::JoinHandle};

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

        let queue = Arc::new(Queue::new());

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
    /// use zero_pool::{ZeroPool, TaskFnPointer, TaskParamPointer, zp_define_task_fn, zp_write};
    ///
    /// struct MyTaskStruct { value: u64, result: *mut u64 }
    ///
    /// zp_define_task_fn!(my_task, MyTaskStruct, |params| {
    ///     zp_write!(params.result, params.value * 2);
    /// });
    ///
    /// let pool = ZeroPool::new();
    /// let mut result = 0u64;
    /// let params = MyTaskStruct { value: 42, result: &mut result };
    /// let future = pool.submit_raw_task(
    ///     my_task as TaskFnPointer,
    ///     &params as *const _ as TaskParamPointer
    /// );
    /// future.wait();
    /// ```
    pub fn submit_raw_task(&self, task_fn: TaskFnPointer, params: TaskParamPointer) -> TaskFuture {
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
    /// use zero_pool::{ZeroPool, uniform_tasks_to_pointers, zp_define_task_fn, zp_write};
    ///
    /// struct MyTaskStruct { value: u64, result: *mut u64 }
    ///
    /// zp_define_task_fn!(my_task, MyTaskStruct, |params| {
    ///     zp_write!(params.result, params.value * 2);
    /// });
    ///
    /// let pool = ZeroPool::new();
    /// let mut results = [0u64; 2];
    /// let params_vec = [
    ///     MyTaskStruct { value: 1, result: &mut results[0] },
    ///     MyTaskStruct { value: 2, result: &mut results[1] }
    /// ];
    /// let tasks = uniform_tasks_to_pointers(my_task, &params_vec);
    /// let future = pool.submit_raw_task_batch(&tasks);
    /// future.wait();
    /// ```
    pub fn submit_raw_task_batch(&self, tasks: &[TaskItem]) -> TaskFuture {
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
    /// use zero_pool::{ZeroPool, zp_define_task_fn, zp_write};
    ///
    /// struct MyTaskStruct { value: u64, result: *mut u64 }
    ///
    /// zp_define_task_fn!(my_task_fn, MyTaskStruct, |params| {
    ///     zp_write!(params.result, params.value * 2);
    /// });
    ///
    /// let pool = ZeroPool::new();
    /// let mut result = 0u64;
    /// let task_params = MyTaskStruct { value: 42, result: &mut result };
    /// let future = pool.submit_task(my_task_fn, &task_params);
    /// future.wait();
    /// assert_eq!(result, 84);
    /// ```
    pub fn submit_task<T>(&self, task_fn: TaskFnPointer, params: &T) -> TaskFuture {
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
    /// use zero_pool::{ZeroPool, zp_define_task_fn, zp_write};
    ///
    /// struct MyTaskStruct { value: u64, result: *mut u64 }
    ///
    /// zp_define_task_fn!(my_task_fn, MyTaskStruct, |params| {
    ///     zp_write!(params.result, params.value * 2);
    /// });
    ///
    /// let pool = ZeroPool::new();
    /// let mut results = vec![0u64; 1000];
    /// let tasks: Vec<_> = (0..1000)
    ///     .map(|i| MyTaskStruct { value: i as u64, result: &mut results[i] })
    ///     .collect();
    /// let future = pool.submit_batch_uniform(my_task_fn, &tasks);
    /// future.wait();
    /// ```
    pub fn submit_batch_uniform<T>(&self, task_fn: TaskFnPointer, params_vec: &[T]) -> TaskFuture {
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
