use std::{
    sync::Arc,
    thread::{self, JoinHandle},
};

use crate::{queue::Queue, task_future::TaskFuture};

pub fn spawn_worker(id: usize, queue: Arc<Queue>) -> JoinHandle<()> {
    thread::Builder::new()
        .name(format!("zp{}", id))
        .spawn(move || {
            let mut current_batch_ptr: *const TaskFuture = std::ptr::null();
            let mut current_batch_count = 0usize;

            loop {
                if queue.wait_for_work() {
                    break;
                }

                while let Some((work_item, batch_future)) = queue.claim_work() {
                    let batch_ptr = batch_future as *const TaskFuture;

                    // check if same batch
                    if current_batch_ptr != batch_ptr {
                        // flush previous batch if exists
                        if current_batch_count > 0 {
                            unsafe { (*current_batch_ptr).complete_many(current_batch_count) };
                            current_batch_count = 0;
                        }
                        current_batch_ptr = batch_ptr;
                    }

                    // execute task
                    (work_item.0)(work_item.1);
                    current_batch_count += 1;
                }

                // flush any remaining completions when no more work
                if current_batch_count > 0 {
                    unsafe { (*current_batch_ptr).complete_many(current_batch_count) };
                    current_batch_count = 0;
                }
            }
        })
        .expect("spawn failed")
}
