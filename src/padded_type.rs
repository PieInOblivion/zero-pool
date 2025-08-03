use std::{ops::{Deref, DerefMut}, sync::atomic::{AtomicPtr, AtomicUsize}};

// an atomic usize padded to fill one cache line, 64 bytes
// this prevents false sharing when multiple threads access different atomics
#[repr(align(64))]
pub struct PaddedType<T, const PAD: usize> {
    value: T,
    // padding to fill the rest of the cache line
    _pad: [u8; PAD],
}

impl<T, const PAD: usize> PaddedType<T, PAD> {
    pub const fn new_padded(value: T) -> Self {
        PaddedType {
            value,
            _pad: [0; PAD],
        }
    }
}

impl<T, const PAD: usize> Deref for PaddedType<T, PAD> {
    type Target = T;

    #[inline]
    fn deref(&self) -> &Self::Target {
        &self.value
    }
}

impl<T, const PAD: usize> DerefMut for PaddedType<T, PAD> {
    #[inline]
    fn deref_mut(&mut self) -> &mut Self::Target {
        &mut self.value
    }
}

pub type PaddedAtomicUsize = PaddedType<AtomicUsize, 56>;
pub type PaddedAtomicPtr<T> = PaddedType<AtomicPtr<T>, 56>;

impl PaddedAtomicUsize {
    pub fn new(value: usize) -> Self {
        Self::new_padded(AtomicUsize::new(value))
    }
}

impl<T> PaddedAtomicPtr<T> {
    pub fn new(ptr: *mut T) -> Self {
        Self::new_padded(AtomicPtr::new(ptr))
    }
}