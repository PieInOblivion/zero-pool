use std::sync::atomic::Ordering;

use crate::{
    future::WorkFuture,
    padded_type::{PaddedAtomicPtr, PaddedAtomicUsize},
    work_item::WorkItem,
};

pub struct WorkBatch {
    next_item: PaddedAtomicUsize,
    pub items: Vec<WorkItem>,
    pub future: WorkFuture,
    pub next: PaddedAtomicPtr<WorkBatch>,
}

impl WorkBatch {
    pub fn new(items: Vec<WorkItem>, future: WorkFuture) -> Self {
        WorkBatch {
            next_item: PaddedAtomicUsize::new(0),
            items,
            future,
            next: PaddedAtomicPtr::new(std::ptr::null_mut()),
        }
    }

    pub fn claim_next_item(&self) -> Option<WorkItem> {
        let item_index = self.next_item.fetch_add(1, Ordering::Relaxed);

        if item_index < self.items.len() {
            Some(self.items[item_index])
        } else {
            None
        }
    }

    pub fn has_work(&self) -> bool {
        self.next_item.load(Ordering::Relaxed) < self.items.len()
    }

    pub fn remaining_items(&self) -> usize {
        let claimed = self.next_item.load(Ordering::Relaxed);
        self.items.len().saturating_sub(claimed)
    }
}
