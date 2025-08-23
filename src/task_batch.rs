use std::sync::atomic::{AtomicPtr, Ordering};

use crate::{TaskItem, padded_type::PaddedAtomicUsize, task_future::TaskFuture};

pub struct TaskBatch {
    next_item: PaddedAtomicUsize,
    items: Vec<TaskItem>,
    pub future: TaskFuture,
    pub next: AtomicPtr<TaskBatch>,
}

impl TaskBatch {
    pub fn new(items: Vec<TaskItem>, future: TaskFuture) -> Self {
        TaskBatch {
            next_item: PaddedAtomicUsize::new(0),
            items,
            future,
            next: AtomicPtr::new(std::ptr::null_mut()),
        }
    }

    pub fn claim_next_item(&self) -> Option<TaskItem> {
        let item_index = self.next_item.fetch_add(1, Ordering::Relaxed);
        self.items.get(item_index).copied()
    }

    pub fn has_tasks(&self) -> bool {
        self.next_item.load(Ordering::Relaxed) < self.items.len()
    }
}
