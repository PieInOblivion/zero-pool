use std::{
    ptr,
    sync::{
        Arc,
        atomic::{AtomicUsize, Ordering},
    },
    thread::{self, JoinHandle, Thread},
};

use crate::{queue::Queue, task_batch::TaskBatch};

const RETIRED_VEC_MIN_CAP: usize = 8;
const RETIRED_VEC_SHRINK_RATIO: usize = 4;
const RETIRED_VEC_SHRINK_CADENCE: u8 = 32;

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
            let mut retired_batches = Vec::with_capacity(RETIRED_VEC_MIN_CAP);
            let mut reclaim_calls: u8 = 0;

            while queue.wait_for_work(id) {
                while let Some((batch, first_param)) =
                    queue.get_next_batch(&mut last_incremented_on, &mut retired_batches)
                {
                    let mut completed = 1;
                    (batch.task_fn_ptr)(first_param);

                    while let Some(param) = batch.claim_next_param() {
                        (batch.task_fn_ptr)(param);
                        completed += 1;
                    }

                    if batch.complete_many(completed) {
                        batch.viewers_decrement();
                        reclaim_local(&mut retired_batches, &mut reclaim_calls);
                    }
                }

                if !last_incremented_on.is_null() {
                    unsafe { (&*last_incremented_on).viewers_decrement() };
                    last_incremented_on = ptr::null_mut();
                }

                reclaim_local(&mut retired_batches, &mut reclaim_calls);
            }

            reclaim_local(&mut retired_batches, &mut reclaim_calls);
        })
        .expect("spawn failed")
}

fn reclaim_local(retired_batches: &mut Vec<*mut TaskBatch>, reclaim_calls: &mut u8) {
    let mut index = 0;

    while index < retired_batches.len() {
        let batch_ptr = retired_batches[index];
        let can_reclaim = unsafe { (&*batch_ptr).viewers_count() == 0 };

        if can_reclaim {
            println!("reclaim_local: {:?}", batch_ptr);
            unsafe {
                drop(Box::from_raw(batch_ptr));
            }
            retired_batches.swap_remove(index);
        } else {
            index += 1;
        }
    }

    *reclaim_calls = reclaim_calls.wrapping_add(1);
    if *reclaim_calls % RETIRED_VEC_SHRINK_CADENCE == 0 {
        let len = retired_batches.len();
        let cap = retired_batches.capacity();
        let baseline = len.max(RETIRED_VEC_MIN_CAP);

        if cap > baseline * RETIRED_VEC_SHRINK_RATIO {
            let target = len.max(RETIRED_VEC_MIN_CAP);
            retired_batches.shrink_to(target);
        }
    }
}
