use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;
use std::thread::{self, JoinHandle};

use crate::queue::WorkQueue;
use crate::work_item::WorkItem;

// Worker thread that processes tasks from its dedicated queue
pub struct Worker {
    _id: usize,
    pub(crate) should_shutdown: Arc<AtomicBool>,
    pub(crate) handle: Option<JoinHandle<()>>,
}

impl Worker {
    pub fn new(id: usize, work_queue: Arc<WorkQueue>) -> Worker {
        let should_shutdown = Arc::new(AtomicBool::new(false));
        let shutdown_flag = Arc::clone(&should_shutdown);

        let handle = thread::Builder::new()
            .name(format!("zero-pool-worker-{}", id))
            .spawn(move || {
                loop {
                    // Wait on condvar
                    work_queue.wait_for_work();

                    // Continue while work queue is not empty
                    while let Some(work_item) = work_queue.try_get_work() {
                        Self::process_work(work_item);
                    }

                    // Check shutdown flag. Better to have an empty queue check when shutdown is required
                    // than to check everytime a worker is woken up
                    if shutdown_flag.load(Ordering::Acquire) {
                        break;
                    }
                }
            })
            .expect("Failed to spawn worker thread");

        Worker {
            _id: id,
            should_shutdown,
            handle: Some(handle),
        }
    }

    // Process a single work item
    fn process_work(work_item: WorkItem) {
        // Call the task function with raw parameters
        (work_item.task_fn)(work_item.params);

        // Signal completion
        work_item.future.complete();
    }
}
