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
                queue.wait_for_signal();

                while let Some((batch, first_param)) = {
                    let global_epoch = queue.update_epoch(id, &mut cached_local_epoch);
                    queue.get_next_batch(global_epoch)
                } {
                    let (params_ptr, param_stride, params_total_bytes, task_fn) =
                        batch.get_run_values();

                    let mut completed = 1;
                    task_fn(first_param);

                    while let Some(param) =
                        batch.claim_next_param_local(params_ptr, param_stride, params_total_bytes)
                    {
                        task_fn(param);
                        completed += 1;
                    }

                    if batch.future.complete_many(completed) && queue.should_reclaim() {
                        queue.reclaim();
                    }
                }

                queue.exit_epoch(id, &mut cached_local_epoch);

                if queue.is_shutdown() {
                    break;
                }
            }
        })
        .expect("spawn failed")
}
