use std::collections::VecDeque;
use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::{Condvar, Mutex};

use crate::work_item::WorkItem;

// Per-worker queue with condvar coordination
// items_count:
// - counter may temporarily read higher than actual queue size
//   - no underflow is possible because of item.is_some() check
// - this is acceptable as spurious checks are harmless compared to performance impact of perfect alignment of seqcst
// - the fix is to have all atomic operations be SeqCst which hurts performance more in all cases
// - will always settle at 0, as all +1 have a -1
pub struct WorkQueue {
    pub queue: Mutex<VecDeque<WorkItem>>,
    pub items_count: AtomicUsize,
    pub condvar: Condvar,
}

impl WorkQueue {
    pub fn new() -> Self {
        WorkQueue {
            queue: Mutex::new(VecDeque::new()),
            items_count: AtomicUsize::new(0),
            condvar: Condvar::new(),
        }
    }

    // Submit a work item to this queue
    pub fn add_work_item(&self, work_item: WorkItem) {
        let mut queue = self.queue.lock().unwrap();
        queue.push_back(work_item);
        self.items_count.fetch_add(1, Ordering::Release);
    }

    // Notify the worker assigned to this queue
    pub fn notify_worker(&self) {
        self.condvar.notify_one();
    }

    pub fn wait_for_work(&self) {
        let mut queue = self.queue.lock().unwrap();
        while self.items_count.load(Ordering::Acquire) == 0 {
            queue = self.condvar.wait(queue).unwrap();
        }
    }

    // Try to get work without blocking
    pub fn try_get_work(&self) -> Option<WorkItem> {
        let mut queue = self.queue.lock().unwrap();
        self.try_pop_work_item(&mut queue)
    }

    fn try_pop_work_item(&self, queue: &mut VecDeque<WorkItem>) -> Option<WorkItem> {
        if self.items_count.load(Ordering::Acquire) > 0 {
            let item = queue.pop_front();
            if item.is_some() {
                self.items_count.fetch_sub(1, Ordering::Release);
            }
            item
        } else {
            None
        }
    }

    // Get queue length
    pub fn len(&self) -> usize {
        self.items_count.load(Ordering::Acquire)
    }
}
