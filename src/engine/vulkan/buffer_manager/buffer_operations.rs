use crate::engine::vulkan::VulkanData;

use super::super::prelude::*;

pub trait BufferAllocator {
    type Output;

    fn allocate_with_size(&mut self, size: u64) -> Result<Self::Output>
    where
        Self: Sized;

    fn allocate<S>(&mut self) -> Result<Self::Output>
    where
        Self: Sized,
    {
        self.allocate_with_size(size_of::<S>() as u64)
    }
}

pub trait BufferOperations {
    type DropData<'a>;

    fn free<'a>(&mut self, drop_data: Self::DropData<'a>);
}
