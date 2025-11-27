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
    type DropData<'a>: Clone;
    type BufferType;

    fn get_buffer(&self) -> Self::BufferType;
    fn get_memory(&self) -> DeviceMemory;

    unsafe fn free<'a>(&mut self, drop_data: Self::DropData<'a>);
}

pub trait SupportsCopying {
    fn copy(
        &mut self,
        destination: Self,
        graphics_queue: Queue,
        command_pool: CommandPool,
        device: Device,
        size: u64,
    ) -> Result<()>;
}
