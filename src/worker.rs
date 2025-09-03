use std::{
    sync::Arc,
    thread::{self, JoinHandle},
};

use crate::queue::Queue;

pub fn spawn_worker(
    id: usize,
    queue: Arc<Queue>,
    barrier: Arc<std::sync::Barrier>,
) -> JoinHandle<()> {
    thread::Builder::new()
        .name(format!("zp{}", id))
        .spawn(move || {
            // register this thread with the queue's waiter so it can be unparked by id
            queue.register_worker_thread(id);
            // signal registration complete and wait for all workers + main
            barrier.wait();

            loop {
                queue.wait_for_signal(id);

                while let Some((batch, first_param, future)) = queue.get_next_batch(id) {
                    let task_fn = batch.task_fn();
                    let mut completed = 1;

                    // process first claimed param
                    task_fn(first_param);

                    while let Some(param) = batch.claim_next_param() {
                        task_fn(param);
                        completed += 1;
                    }

                    if future.complete_many(completed) {
                        queue.try_reclaim();
                    }
                }

                if queue.is_shutdown() {
                    break;
                }
            }
        })
        .expect("spawn failed")
}
