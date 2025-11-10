use std::{
    mem::MaybeUninit,
    ops::{Deref, DerefMut},
};

/// Struct to have variables be uninitialized at first and then initialized later
///
/// Safe wrapper around `MaybeUninit<T>`
pub struct Init<T> {
    value: MaybeUninit<T>,
    initialized: bool,
}

impl<T> Init<T> {
    pub fn uninit() -> Self {
        Self {
            value: MaybeUninit::uninit(),
            initialized: false,
        }
    }

    /// Should be safe? Honestly I'm not sure.
    ///
    /// This function casts the internal contents to a mut ptr and derefs to set the value.
    pub fn init(&mut self, value: T) {
        self.value.write(value);
        self.initialized = true;
    }
}

impl<T> Deref for Init<T> {
    type Target = T;

    fn deref(&self) -> &Self::Target {
        if self.initialized {
            return unsafe { self.value.assume_init_ref() };
        }
        panic!("Tried to dereference an uninitialized variable")
    }
}

impl<T> DerefMut for Init<T> {
    fn deref_mut(&mut self) -> &mut T {
        if self.initialized {
            return unsafe { self.value.assume_init_mut() };
        }
        panic!("Tried to dereference an uninitialized variable")
    }
}
