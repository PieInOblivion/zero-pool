use std::{
    ptr,
    sync::Arc,
    thread::{self, JoinHandle},
};

use crate::{queue::Queue, startup_latch::LatchSignaler, task_batch::TaskBatch};

pub fn spawn_worker(id: usize, queue: Arc<Queue>, signaler: LatchSignaler) -> JoinHandle<()> {
    thread::Builder::new()
        .name(format!("zp{}", id))
        .spawn(move || {
            // register this thread with the queue's waiter so it can be unparked by id
            queue.register_worker_thread(id);

            // signal registration complete
            signaler.signal();

            let mut last_incremented_on = ptr::null_mut();

            while queue.wait_for_work(id) {
                while let Some((batch, first_param)) =
                    queue.get_next_batch(&mut last_incremented_on)
                {
                    let mut completed = 1;
                    (batch.task_fn_ptr)(first_param);

                    while let Some(param) = batch.claim_next_param() {
                        (batch.task_fn_ptr)(param);
                        completed += 1;
                    }

                    if batch.complete_many(completed) {
                        unsafe {
                            TaskBatch::release_ptr(batch as *const TaskBatch as *mut TaskBatch);
                        }
                    }
                }

                if !last_incremented_on.is_null() {
                    unsafe { TaskBatch::release_ptr(last_incremented_on) };
                    last_incremented_on = ptr::null_mut();
                }
            }
        })
        .expect("spawn failed")
}
