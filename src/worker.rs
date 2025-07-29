use std::{
    sync::Arc,
    thread::{self, JoinHandle},
};

use crate::{future::WorkFuture, queue::BatchQueue, work_item::WorkItem};

pub struct Worker {
    _id: usize,
    pub(crate) handle: JoinHandle<()>,
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
                        Self::process_work(work_item, batch_future);
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

    #[inline]
    fn process_work(work_item: &WorkItem, batch_future: &WorkFuture) {
        // call the task function with raw parameters
        (work_item.task_fn)(work_item.params);

        batch_future.complete_one();
    }
}
