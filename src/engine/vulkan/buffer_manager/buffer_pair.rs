use crate::engine::vulkan::{
    VulkanData,
    buffer_manager::buffer_operations::{BufferAllocator, BufferOperations, SupportsCopying},
};
use std::fmt::Display;

use super::super::prelude::*;

#[derive(Default, Debug, Clone, Copy, Eq, PartialEq, Hash)]
pub struct BufferPair {
    pub buffer: Buffer,
    pub memory: DeviceMemory,
}

pub struct BufferPairData<'a> {
    pub instance: &'a Instance,
    pub device: &'a Device,
    pub physical_device: PhysicalDevice,
    pub usage: BufferUsageFlags,
    pub properties: MemoryPropertyFlags,
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
    type DropData<'a> = Device;
    type BufferType = Buffer;

    fn get_buffer(&self) -> Self::BufferType {
        self.buffer.clone()
    }

    fn get_memory(&self) -> DeviceMemory {
        self.memory.clone()
    }

    unsafe fn free<'a>(&mut self, device: Self::DropData<'a>) {
        unsafe { device.destroy_buffer(self.buffer, None) };
        unsafe { device.free_memory(self.memory, None) };
    }
}

impl SupportsCopying for BufferPair {
    fn copy(
        &mut self,
        destination: Self,
        graphics_queue: Queue,
        command_pool: CommandPool,
        device: Device,
        size: u64,
    ) -> Result<()> {
        unsafe {
            device.unmap_memory(self.memory);

            copy_buffer(
                &device,
                graphics_queue,
                command_pool,
                self.buffer,
                destination.buffer,
                size,
            )?;
        }

        Ok(())
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

#[derive(Default, Debug, Clone, Copy, Eq, PartialEq, Hash)]
pub enum StandardBufferMaps {
    #[default]
    Vertices,
    Indices,
    ExtraVertices(usize),
    ExtraIndices(usize),
    GuiVertices(usize),
    GuiIndices(usize),
}

#[derive(Default, Debug, Clone, Copy, Eq, PartialEq, Hash)]
pub enum UniformBufferMaps {
    #[default]
    ModelViewProject,
    SporadicBufferObject,
    TextureSampler,
}

impl Display for StandardBufferMaps {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            StandardBufferMaps::Vertices => f.write_str("StandardBufferMaps::Vertices"),
            StandardBufferMaps::Indices => f.write_str("StandardBufferMaps::Indices"),
            StandardBufferMaps::ExtraVertices(_) => {
                f.write_str("StandardBufferMaps::ExtraVertices")
            }
            StandardBufferMaps::ExtraIndices(_) => f.write_str("StandardBufferMaps::ExtraIndices"),
            StandardBufferMaps::GuiVertices(_) => f.write_str("StandardBufferMaps::GuiVertices"),
            StandardBufferMaps::GuiIndices(_) => f.write_str("StandardBufferMaps::GuiIndices"),
        }
    }
}
impl Display for UniformBufferMaps {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            UniformBufferMaps::ModelViewProject => {
                f.write_str("UniformBufferMaps::ModelViewProject")
            }
            UniformBufferMaps::SporadicBufferObject => {
                f.write_str("UniformBufferMaps::SporadicBufferObject")
            }
            UniformBufferMaps::TextureSampler => f.write_str("UniformBufferMaps::TextureSampler"),
        }
    }
}
