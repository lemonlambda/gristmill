use crate::engine::vulkan::{
    VulkanData,
    buffer_manager::buffer_operations::{BufferAllocator, BufferOperations},
};

use super::super::prelude::*;

#[derive(Default, Debug, Clone, Copy, PartialEq)]
pub struct BufferPair {
    pub buffer: Buffer,
    pub memory: DeviceMemory,
}

pub struct BufferPairData<'a> {
    instance: &'a Instance,
    device: &'a Device,
    physical_device: PhysicalDevice,
    usage: BufferUsageFlags,
    properties: MemoryPropertyFlags,
}

impl<'a> BufferAllocator for BufferPairData<'a> {
    type Output = BufferPair;

    fn allocate_with_size(&mut self, size: u64) -> Result<Self::Output>
    where
        Self: Sized,
    {
        debug!("Allocating a buffer");
        let buffer_info = BufferCreateInfo::builder()
            .size(size)
            .usage(self.usage)
            .sharing_mode(SharingMode::EXCLUSIVE);

        let buffer = unsafe { self.device.create_buffer(&buffer_info, None) }?;

        let requirements = unsafe { self.device.get_buffer_memory_requirements(buffer) };
        debug!(
            "Buffer Requirement Size: {}, with input size {}",
            requirements.size, size
        );

        let memory_info = MemoryAllocateInfo::builder()
            .allocation_size(requirements.size)
            .memory_type_index(unsafe {
                get_memory_type_index(
                    self.instance,
                    self.physical_device,
                    self.properties,
                    requirements,
                )
            }?);

        let buffer_memory = unsafe { self.device.allocate_memory(&memory_info, None) }?;

        (unsafe { self.device.bind_buffer_memory(buffer, buffer_memory, 0) })?;

        Ok(BufferPair {
            buffer,
            memory: buffer_memory,
        })
    }
}

impl BufferPair {
    pub fn new(buffer: Buffer, memory: DeviceMemory) -> Self {
        Self { buffer, memory }
    }

    pub fn split(&self) -> (Buffer, DeviceMemory) {
        (self.buffer, self.memory)
    }
}

impl BufferOperations for BufferPair {
    type DropData<'a> = &'a mut Device;

    unsafe fn free<'a>(&mut self, device: Self::DropData<'a>) {
        unsafe { device.destroy_buffer(self.buffer, None) };
        unsafe { device.free_memory(self.memory, None) };
    }
}

pub unsafe fn create_buffer(
    instance: &Instance,
    device: &Device,
    data: &VulkanData,
    size: DeviceSize,
    usage: BufferUsageFlags,
    properties: MemoryPropertyFlags,
) -> Result<(Buffer, DeviceMemory)> {
    let buffer_info = BufferCreateInfo::builder()
        .size(size)
        .usage(usage)
        .sharing_mode(SharingMode::EXCLUSIVE);

    let buffer = unsafe { device.create_buffer(&buffer_info, None) }?;

    let requirements = unsafe { device.get_buffer_memory_requirements(buffer) };

    let memory_info = MemoryAllocateInfo::builder()
        .allocation_size(requirements.size)
        .memory_type_index(unsafe {
            get_memory_type_index(instance, data.physical_device, properties, requirements)
        }?);

    let buffer_memory = unsafe { device.allocate_memory(&memory_info, None) }?;

    (unsafe { device.bind_buffer_memory(buffer, buffer_memory, 0) })?;

    Ok((buffer, buffer_memory))
}
