use std::{
    sync::Arc,
    thread::{self, JoinHandle},
};

use crate::{
    garbage_node::GarbageNode,
    queue::{EPOCH_MASK, EPOCH_MASK_HALF, NOT_IN_CRITICAL, Queue},
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
            let mut garbage_head: *mut GarbageNode = std::ptr::null_mut();
            let mut garbage_tail: *mut GarbageNode = std::ptr::null_mut();
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
                    (batch.fn_ptr)(first_param);

                    while let Some(param) = batch.claim_next_param() {
                        (batch.fn_ptr)(param);
                        completed += 1;
                    }

                    batch.future.complete_many(completed);

                    maybe_clean_local_garbage(
                        &queue,
                        &mut local_tick,
                        &mut garbage_head,
                        &mut garbage_tail,
                    );
                }
            }

            // worker thread exits
            drain_garbage(&mut garbage_head);
        })
        .expect("spawn failed")
}

fn drain_garbage(garbage_head: &mut *mut GarbageNode) {
    let mut current = *garbage_head;
    while !current.is_null() {
        current = GarbageNode::drop_node(current);
    }
    *garbage_head = std::ptr::null_mut();
}

fn maybe_clean_local_garbage(
    queue: &Queue,
    local_tick: &mut u8,
    garbage_head: &mut *mut GarbageNode,
    garbage_tail: &mut *mut GarbageNode,
) {
    *local_tick = local_tick.wrapping_add(1);
    if *local_tick != 0 {
        return;
    }

    clean_local_garbage(queue, garbage_head, garbage_tail);
}

fn clean_local_garbage(
    queue: &Queue,
    garbage_head: &mut *mut GarbageNode,
    garbage_tail: &mut *mut GarbageNode,
) {
    let safe_epoch = queue.advance_and_min_epoch();
    let mut current = *garbage_head;

    // list is chronologically sorted; reclaim prefix only
    while !current.is_null() {
        let node_epoch = unsafe { (*current).epoch };
        if safe_epoch.wrapping_sub(node_epoch).wrapping_sub(1) & EPOCH_MASK < (EPOCH_MASK_HALF - 1)
        {
            current = GarbageNode::drop_node(current);
        } else {
            break;
        }
    }

    *garbage_head = current;
    if current.is_null() {
        *garbage_tail = std::ptr::null_mut();
    }
}
