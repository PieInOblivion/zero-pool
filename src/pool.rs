use std::sync::{
    Arc,
    atomic::{AtomicUsize, Ordering},
};

use crate::TaskFn;
use crate::future::{WorkFuture, WorkFutureBatch};
use crate::queue::WorkQueue;
use crate::work_item::WorkItem;
use crate::worker::Worker;

// Thread pool with per-worker queues
pub struct ThreadPool {
    workers: Vec<Worker>,
    work_queues: Vec<Arc<WorkQueue>>,
    next_queue: AtomicUsize,
}

impl ThreadPool {
    // Create a thread pool with the specified number of workers
    pub fn new(worker_count: usize) -> Self {
        assert!(worker_count > 0, "Must have at least one worker");

        // Create per-worker queues
        let work_queues: Vec<Arc<WorkQueue>> = (0..worker_count)
            .map(|_| Arc::new(WorkQueue::new()))
            .collect();

        // Create workers
        let workers: Vec<Worker> = work_queues
            .iter()
            .enumerate()
            .map(|(id, queue)| Worker::new(id, Arc::clone(queue)))
            .collect();

        ThreadPool {
            workers,
            work_queues,
            next_queue: AtomicUsize::new(0),
        }
    }

    // Submit a task with raw pointer parameters
    //
    // Safety
    // - 'params' must point to valid data until 'future.wait()' completes
    // - 'task_fn' must be safe to call with the provided parameters
    pub fn submit_task(&self, params: *const (), task_fn: TaskFn) -> WorkFuture {
        let future = WorkFuture::new();
        let work_item = WorkItem::new(params, task_fn, future.clone());

        // Round-robin distribution across worker queues
        let queue_idx = self.next_queue.fetch_add(1, Ordering::Relaxed) % self.work_queues.len();

        // Submit to specific worker queue
        self.work_queues[queue_idx].add_work_item(work_item);
        self.work_queues[queue_idx].notify_worker();

        future
    }

    // Submit multiple tasks as a batch
    pub fn submit_batch(&self, tasks: Vec<(*const (), TaskFn)>) -> WorkFutureBatch {
        if tasks.is_empty() {
            return WorkFutureBatch::new(Vec::new());
        }

        let mut futures = Vec::with_capacity(tasks.len());

        // Submit all work items first without any notifications
        for (params, task_fn) in tasks {
            let future = WorkFuture::new();
            let work_item = WorkItem::new(params, task_fn, future.clone());

            let queue_idx =
                self.next_queue.fetch_add(1, Ordering::Relaxed) % self.work_queues.len();

            // Submit without immediate notification
            self.work_queues[queue_idx].add_work_item(work_item);
            futures.push(future);
        }

        // Notify workers whose queues have work
        // Alternative approach idea: track which workers recieved work with vec<bool>, and notify using that vec
        // Alternative modification: first check if worker has work in queue already, then don't notify.
        // Notifying condvar that isn't waiting should have nearly no impact versus checking an atomic first. Needs comparing
        for queue in &self.work_queues {
            if queue.items_count.load(Ordering::Acquire) > 0 {
                queue.notify_worker();
            }
        }

        WorkFutureBatch::new(futures)
    }

    // Get the number of workers
    pub fn worker_count(&self) -> usize {
        self.workers.len()
    }

    // Get queue lengths
    pub fn queue_lengths(&self) -> Vec<usize> {
        self.work_queues.iter().map(|queue| queue.len()).collect()
    }

    // Get total pending work items
    pub fn total_pending(&self) -> usize {
        self.work_queues.iter().map(|queue| queue.len()).sum()
    }
}

impl Drop for ThreadPool {
    fn drop(&mut self) {
        // Signal all workers to shutdown
        for (worker, queue) in self.workers.iter().zip(self.work_queues.iter()) {
            // Clear each worker queue first
            {
                let mut q = queue.queue.lock().unwrap();
                q.clear();
                queue.items_count.store(0, Ordering::Release);
            }
            
            // Signal worker to shutdown
            worker.should_shutdown.store(true, Ordering::Release);
            
            // Notify worker to wake up which causes a shutdown flag check
            queue.items_count.fetch_add(1, Ordering::Release);
            queue.notify_worker();
        }
        
        // Wait for all workers to finish
        for mut worker in self.workers.drain(..) {
            if let Some(handle) = worker.handle.take() {
                if let Err(e) = handle.join() {
                    eprintln!("Worker thread panicked during shutdown: {:?}", e);
                }
            }
        }
    }
}