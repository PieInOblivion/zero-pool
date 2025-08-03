use std::sync::atomic::{AtomicUsize, Ordering};

// an atomic usize padded to fill one cache line, 64 bytes
// this prevents false sharing when multiple threads access different atomics
#[repr(align(64))]
pub struct PaddedAtomicUsize {
    value: AtomicUsize,
    // padding to fill the rest of the cache line
    _pad: [u8; 64 - std::mem::size_of::<AtomicUsize>()],
}

impl PaddedAtomicUsize {
    pub fn new(value: usize) -> Self {
        PaddedAtomicUsize {
            value: AtomicUsize::new(value),
            _pad: [0; 64 - std::mem::size_of::<AtomicUsize>()],
        }
    }

    #[inline]
    pub fn load(&self, order: Ordering) -> usize {
        self.value.load(order)
    }

    #[inline]
    pub fn fetch_add(&self, val: usize, order: Ordering) -> usize {
        self.value.fetch_add(val, order)
    }

    #[inline]
    pub fn fetch_sub(&self, val: usize, order: Ordering) -> usize {
        self.value.fetch_sub(val, order)
    }
}