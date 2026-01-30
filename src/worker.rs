use std::{
    sync::{Arc, Barrier},
    thread::{self, JoinHandle},
};

use crate::queue::{NOT_IN_CRITICAL, Queue};

pub fn spawn_worker(id: usize, queue: Arc<Queue>, barrier: Arc<Barrier>) -> JoinHandle<()> {
    thread::Builder::new()
        .name(format!("zp{}", id))
        .spawn(move || {
            // register this thread with the queue's waiter so it can be unparked by id
            queue.register_worker_thread(id);
            // signal registration complete and wait for all workers + main
            barrier.wait();
            drop(barrier);

            let mut cached_local_epoch = NOT_IN_CRITICAL;

            loop {
                if !queue.wait_for_work(id, &mut cached_local_epoch) {
                    break;
                }

                while let Some((batch, first_param)) =
                    queue.get_next_batch(id, &mut cached_local_epoch)
                {
                    let mut completed = 1;
                    (batch.task_fn_ptr)(first_param);

                    while let Some(param) = batch.claim_next_param() {
                        (batch.task_fn_ptr)(param);
                        completed += 1;
                    }

                    if batch.future.complete_many(completed) && queue.should_reclaim() {
                        queue.reclaim();
                    }
                }
            }
        })
        .expect("spawn failed")
}
