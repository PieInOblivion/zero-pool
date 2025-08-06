use std::sync::atomic::Ordering;

use crate::{
    WorkItem,
    future::WorkFuture,
    padded_type::{PaddedAtomicPtr, PaddedAtomicUsize},
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

    #[inline(always)]
    pub fn claim_next_item(&self) -> Option<WorkItem> {
        let item_index = self.next_item.fetch_add(1, Ordering::Relaxed);
        self.items.get(item_index).copied()
    }

    pub fn has_work(&self) -> bool {
        self.next_item.load(Ordering::Relaxed) < self.items.len()
    }

    pub fn remaining_items(&self) -> usize {
        let claimed = self.next_item.load(Ordering::Relaxed);
        self.items.len().saturating_sub(claimed)
    }
}
