use std::collections::HashMap;

use anyhow::Result;
use std::hash::Hash;
use vulkanalia::Device;
use vulkanalia::vk::*;

pub enum StandardBufferMaps {
    Vertices,
    Indices,
}

pub enum UniformBufferMaps {
    ModelViewProject,
    SporadicBufferObject,
    TextureSampler,
}

pub trait BufferManagerRequirements = Default + Eq + Hash;

#[derive(Default, Clone)]
pub struct BufferManager<S: BufferManagerRequirements, U: BufferManagerRequirements> {
    buffers: HashMap<S, BufferPair>,
    uniform_buffers: HashMap<U, Vec<BufferPair>>,
}

impl<S: BufferManagerRequirements, U: BufferManagerRequirements> BufferManager<S, U> {
    pub fn new() -> Self {
        Self::default()
    }

    unsafe fn create_buffer_descriptor_set<'a, UBO>(
        binding: u32,
        buffer: Buffer,
        descriptor_set: DescriptorSet,
    ) -> WriteDescriptorSetBuilder<'a> {
        let info = DescriptorBufferInfo::builder()
            .buffer(buffer)
            .offset(0)
            .range(size_of::<UBO>() as u64);

        let buffer_info = Box::new([info]);
        let buffer_info: &'a mut _ = Box::leak(buffer_info);

        WriteDescriptorSet::builder()
            .dst_set(descriptor_set)
            .dst_binding(binding)
            .dst_array_element(0)
            .descriptor_type(DescriptorType::UNIFORM_BUFFER)
            .buffer_info(buffer_info)
    }

    pub unsafe fn allocate_new_uniform_buffer(
        &mut self,
        name: U,
        device: &Device,
        size: DeviceSize,
        usage: BufferUsageFlags,
    ) -> Result<()> {
        let buffer_info = BufferCreateInfo::builder()
            .size(size)
            .usage(usage)
            .sharing_mode(SharingMode::EXCLUSIVE);

        let buffer = unsafe { device.create_buffer(&buffer_info, None) }?;

        let requirements = unsafe { device.get_buffer_memory_requirements(buffer) };

        let memory_info = MemoryAllocateInfo::builder()
            .allocation_size(requirements.size)
            .memory_type_index(unsafe {
                Self::get_memory_type_index(instance, data, properties, requirements)
            }?);

        let buffer_memory = unsafe { device.allocate_memory(&memory_info, None) }?;

        (unsafe { device.bind_buffer_memory(buffer, buffer_memory, 0) })?;

        self.uniform_buffers
            .entry(name)
            .and_modify(|v| v.push(BufferPair::new(buffer, memory)))
            .or_insert(vec![BufferPair::new(buffer, buffer_memory)]);

        Ok(())
    }
}

#[derive(Default, Clone, Copy)]
struct BufferPair(Buffer, DeviceMemory);

impl BufferPair {
    pub fn new(buffer: Buffer, memory: DeviceMemory) -> Self {
        Self(buffer, memory)
    }
}
