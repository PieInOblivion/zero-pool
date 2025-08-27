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
                while let Some((batch, first_param, future)) = queue.get_next_batch() {
                    let task_fn = batch.task_fn();
                    let mut completed = 1;
                    // process first claimed param
                    task_fn(first_param);
                    
                    while let Some(param) = batch.claim_next_param() {
                        task_fn(param);
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
