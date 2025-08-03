use std::{
    sync::Arc,
    thread::{self, JoinHandle},
};

use crate::queue::BatchQueue;

pub struct Worker {
    _id: usize,
    pub handle: JoinHandle<()>,
}

impl Worker {
    pub fn new(id: usize, queue: Arc<BatchQueue>) -> Worker {
        let handle = thread::Builder::new()
            .name(format!("zero-pool-worker-{}", id))
            .spawn(move || {
                loop {
                    // wait for work
                    queue.wait_for_work();

                    // once woken up, greedily drain ALL available work
                    while let Some((work_item, batch_future)) = queue.claim_work() {
                        // call the task function with raw parameters
                        (work_item.task_fn)(work_item.params);
                        batch_future.complete_one();
                    }

                    // only check shutdown when queue is empty
                    if queue.is_shutdown() {
                        break;
                    }
                }
            })
            .expect("Failed to spawn worker thread");

        Worker {
            _id: id,
            handle: handle,
        }
    }
}
