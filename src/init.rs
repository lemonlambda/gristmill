use std::ops::{Deref, DerefMut};

/// Struct to have variables be uninitialized at first and then initialized later
pub struct Init<T> {
    value: Option<T>,
}

impl<T> Init<T> {
    pub fn blank() -> Self {
        Self { value: None }
    }

    pub fn init(&mut self, value: T) {
        self.value = Some(value)
    }
}

impl<T> Deref for Init<T> {
    type Target = T;

    fn deref(&self) -> &Self::Target {
        self.value.as_ref().unwrap()
    }
}

impl<T> DerefMut for Init<T> {
    fn deref_mut(&mut self) -> &mut T {
        self.value.as_mut().unwrap()
    }
}
