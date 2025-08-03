use std::sync::atomic::{AtomicPtr, Ordering};

use crate::{future::WorkFuture, padded_atomic::PaddedAtomicUsize, work_item::WorkItem};

#[repr(align(64))]
pub struct WorkBatch {
    pub items: Vec<WorkItem>,
    pub future: WorkFuture,
    pub next: AtomicPtr<WorkBatch>,
    next_item: PaddedAtomicUsize,
}

impl WorkBatch {
    pub fn new(items: Vec<WorkItem>, future: WorkFuture) -> Self {
        WorkBatch {
            items,
            future,
            next: AtomicPtr::new(std::ptr::null_mut()),
            next_item: PaddedAtomicUsize::new(0),
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