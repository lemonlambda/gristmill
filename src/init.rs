use std::{
    mem::MaybeUninit,
    ops::{Deref, DerefMut},
    ptr::NonNull,
};

/// Struct to have variables be uninitialized at first and then initialized later
///
/// Safe wrapper around `MaybeUninit<T>`
pub struct Init<T> {
    value: MaybeUninit<T>,
}

impl<T> Init<T> {
    pub fn uninit() -> Self {
        Self {
            value: MaybeUninit::uninit(),
        }
    }

    /// Should be safe? Honestly I'm not sure.
    ///
    /// This function casts the internal contents to a mut ptr and derefs to set the value.
    pub fn init(&mut self, value: T) {
        unsafe {
            *self.value.as_mut_ptr() = value;
        }
    }
}

impl<T> Deref for Init<T> {
    type Target = T;

    fn deref(&self) -> &Self::Target {
        unsafe { self.value.assume_init_ref() }
    }
}

impl<T> DerefMut for Init<T> {
    fn deref_mut(&mut self) -> &mut T {
        unsafe { self.value.assume_init_mut() }
    }
}
