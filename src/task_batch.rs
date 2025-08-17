use std::sync::atomic::Ordering;

use crate::{
    TaskItem,
    future::WorkFuture,
    padded_type::{PaddedAtomicPtr, PaddedAtomicUsize},
};

pub struct TaskBatch {
    next_item: PaddedAtomicUsize,
    pub items: Vec<TaskItem>,
    pub future: WorkFuture,
    pub next: PaddedAtomicPtr<TaskBatch>,
}

impl TaskBatch {
    pub fn new(items: Vec<TaskItem>, future: WorkFuture) -> Self {
        TaskBatch {
            next_item: PaddedAtomicUsize::new(0),
            items,
            future,
            next: PaddedAtomicPtr::new(std::ptr::null_mut()),
        }
    }

    #[inline]
    pub fn claim_next_item(&self) -> Option<TaskItem> {
        let item_index = self.next_item.fetch_add(1, Ordering::Relaxed);
        self.items.get(item_index).copied()
    }

    #[inline]
    pub fn has_work(&self) -> bool {
        self.next_item.load(Ordering::Relaxed) < self.items.len()
    }
}
