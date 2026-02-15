use std::{
    ptr,
    sync::{
        Arc,
        atomic::{AtomicUsize, Ordering},
    },
    thread::{self, JoinHandle, Thread},
};

use crate::queue::Queue;

pub fn spawn_worker(
    id: usize,
    queue: Arc<Queue>,
    latch_thread: Thread,
    latch_count_ptr: usize,
) -> JoinHandle<()> {
    thread::Builder::new()
        .name(format!("zp{}", id))
        .spawn(move || {
            // register this thread with the queue's waiter so it can be unparked by id
            queue.register_worker_thread(id);
            // signal registration complete
            let latch_count = unsafe { &*(latch_count_ptr as *const AtomicUsize) };
            if latch_count.fetch_sub(1, Ordering::Release) == 1 {
                latch_thread.unpark();
            }
            drop(latch_thread);

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

                    batch.complete_many(completed);
                }

                if !last_incremented_on.is_null() {
                    queue.release_viewer(last_incremented_on);
                    last_incremented_on = ptr::null_mut();
                }
            }
        })
        .expect("spawn failed")
}
