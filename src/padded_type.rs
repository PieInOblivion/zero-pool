use std::ops::{Deref, DerefMut};

// a generic type padded to fill one cache line, 64 bytes
#[repr(align(64))]
pub struct PaddedType<T> {
    value: T,
}

impl<T> PaddedType<T> {
    pub const fn new(value: T) -> Self {
        PaddedType { value }
    }
}

impl<T> Deref for PaddedType<T> {
    type Target = T;

    #[inline]
    fn deref(&self) -> &Self::Target {
        &self.value
    }
}

impl<T> DerefMut for PaddedType<T> {
    #[inline]
    fn deref_mut(&mut self) -> &mut Self::Target {
        &mut self.value
    }
}
