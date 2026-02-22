use std::{
    sync::Arc,
    thread::{self, JoinHandle},
};

use crate::{
    queue::{EPOCH_MASK, NOT_IN_CRITICAL, Queue},
    task_batch::TaskBatch,
    task_future::TaskFuture,
};

pub fn spawn_worker(id: usize, queue: Arc<Queue>, latch: TaskFuture) -> JoinHandle<()> {
    thread::Builder::new()
        .name(format!("zp{}", id))
        .spawn(move || {
            // register this thread with the queue's waiter so it can be unparked by id
            queue.register_worker_thread(id);
            // signal registration complete and wait for all workers + main
            latch.complete_many(1);
            drop(latch);

            let mut cached_local_epoch = NOT_IN_CRITICAL;
            let mut garbage_head: *mut TaskBatch = std::ptr::null_mut();
            let mut garbage_tail: *mut TaskBatch = std::ptr::null_mut();
            let mut local_tick: u8 = 0;

            loop {
                if !queue.wait_for_work(id, &mut cached_local_epoch) {
                    break;
                }

                while let Some((batch, first_param)) = queue.get_next_batch(
                    id,
                    &mut cached_local_epoch,
                    &mut garbage_head,
                    &mut garbage_tail,
                ) {
                    let mut completed = 1;
                    (batch.task_fn_ptr)(first_param);

                    while let Some(param) = batch.claim_next_param() {
                        (batch.task_fn_ptr)(param);
                        completed += 1;
                    }

                    if batch.future.complete_many(completed) {
                        queue.batch_completed();
                    }

                    sweep_local_garbage(
                        &queue,
                        &mut local_tick,
                        &mut garbage_head,
                        &mut garbage_tail,
                    );
                }
            }

            // Drain local garbage_head list when worker thread exits
            drain_garbage(&mut garbage_head);
        })
        .expect("spawn failed")
}

fn drain_garbage(garbage_head: &mut *mut TaskBatch) {
    let mut current = *garbage_head;
    while !current.is_null() {
        let next = unsafe { (*current).local_garbage_next };
        unsafe {
            drop(Box::from_raw(current));
        }
        current = next;
    }
    *garbage_head = std::ptr::null_mut();
}

fn sweep_local_garbage(
    queue: &Queue,
    local_tick: &mut u8,
    garbage_head: &mut *mut TaskBatch,
    garbage_tail: &mut *mut TaskBatch,
) {
    *local_tick = local_tick.wrapping_add(1);
    if *local_tick != 0 {
        return;
    }

    let safe_epoch = queue.min_active_epoch();
    let mut current = *garbage_head;

    // Sweep local intrusive list
    while !current.is_null() {
        let node_epoch = unsafe { (*current).epoch };

        // If the node is older than the minimum active epoch, it's safe to drop
        if safe_epoch.wrapping_sub(node_epoch) & EPOCH_MASK > 0
            && safe_epoch.wrapping_sub(node_epoch) & EPOCH_MASK < (EPOCH_MASK / 2)
        {
            let next = unsafe { (*current).local_garbage_next };
            unsafe {
                drop(Box::from_raw(current));
            }
            current = next;
        } else {
            // List is chronologically sorted; if this node isn't safe, nothing after it is.
            break;
        }
    }

    *garbage_head = current;
    if garbage_head.is_null() {
        *garbage_tail = std::ptr::null_mut();
    }
}
