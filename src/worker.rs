use std::{
    sync::Arc,
    thread::{self, JoinHandle},
};

use crate::queue::Queue;

pub fn spawn_worker(id: usize, queue: Arc<Queue>) -> JoinHandle<()> {
    thread::Builder::new()
        .name(format!("zp{}", id))
        .spawn(move || {
            loop {
                while let Some((batch_ptr, first_item, future)) = queue.get_next_batch() {
                    // process first claimed item
                    (first_item.0)(first_item.1);
                    let mut completed = 1;

                    let batch_ref = unsafe { &*batch_ptr };

                    while let Some(item) = batch_ref.claim_next_item() {
                        (item.0)(item.1);
                        completed += 1;
                    }

                    future.complete_many(completed);
                }

                if queue.wait_for_signal() {
                    break;
                }
            }
        })
        .expect("spawn failed")
}
