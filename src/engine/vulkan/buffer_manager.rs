use std::collections::HashMap;
use std::fmt::Debug;

use anyhow::Result;
use log::*;
use std::hash::Hash;
use vulkanalia::vk::*;
use vulkanalia::{Device, Instance};

use crate::engine::vulkan::VulkanData;
use crate::engine::vulkan::shared_helpers::get_memory_type_index;

#[derive(Default, Debug, Clone, Copy, Eq, PartialEq, Hash)]
pub enum StandardBufferMaps {
    #[default]
    Vertices,
    Indices,
}

#[derive(Default, Debug, Clone, Copy, Eq, PartialEq, Hash)]
pub enum UniformBufferMaps {
    #[default]
    ModelViewProject,
    SporadicBufferObject,
    TextureSampler,
}

pub trait BufferManagerRequirements = Default + Eq + Hash + Debug;

// Buffer Manager needs the following features
//
// [x] Buffer Creation
// [x] Descriptor Sets
// [x] Descriptor Pool
// [ ] Bindings
// [ ] Easy copy to function

#[derive(Clone, Default, Debug)]
pub struct BufferManager<S: BufferManagerRequirements, U: BufferManagerRequirements> {
    instance: Option<Instance>,
    device: Option<Device>,
    physical_device: PhysicalDevice,
    pub temp_buffer: Option<BufferPair>,
    pub buffers: HashMap<S, BufferPair>,
    pub uniform_buffers: HashMap<U, Vec<BufferPair>>,
}

impl<S: BufferManagerRequirements, U: BufferManagerRequirements> BufferManager<S, U> {
    pub fn new(instance: Instance, device: Device, physical_device: PhysicalDevice) -> Self {
        let mut self_ = Self::default();
        self_.instance = Some(instance);
        self_.device = Some(device);
        self_.physical_device = physical_device;
        self_
    }

    pub fn add_instance(&mut self, instance: Instance) -> &mut Self {
        self.instance = Some(instance);
        self
    }
    pub fn add_device(&mut self, device: Device) -> &mut Self {
        self.device = Some(device);
        self
    }
    pub fn add_physical_device(&mut self, physical_device: PhysicalDevice) -> &mut Self {
        self.physical_device = physical_device;
        self
    }

    /// Internal function to skip writing out unwrap
    fn instance(&self) -> Instance {
        self.instance.clone().unwrap()
    }
    /// Internal function to skip writing out unwrap
    fn device(&self) -> Device {
        self.device.clone().unwrap()
    }

    pub unsafe fn create_descriptor_pool<'a>(
        &mut self,
        length: u32,
        additional_sizes: Option<Vec<DescriptorPoolSizeBuilder>>,
    ) -> Result<DescriptorPool> {
        let mut pool_sizes = additional_sizes.unwrap_or(vec![]);

        for _ in 0..self.uniform_buffers.len() {
            let buffer_size = DescriptorPoolSize::builder()
                .type_(DescriptorType::UNIFORM_BUFFER)
                .descriptor_count(length);

            pool_sizes.push(buffer_size);
        }

        let info = DescriptorPoolCreateInfo::builder()
            .pool_sizes(&pool_sizes)
            .max_sets(length);

        Ok(unsafe { self.device().create_descriptor_pool(&info, None) }?)
    }

    /// Frees the previous temp buffer if it exists
    pub unsafe fn create_temp_buffer<B>(
        &mut self,
        usage: BufferUsageFlags,
        properties: MemoryPropertyFlags,
    ) -> Result<()> {
        unsafe { self.free_temp_buffer() }; // Make sure no temp buffer exists already

        self.temp_buffer = Some(BufferPair::allocate::<B>(
            &self.instance(),
            &self.device(),
            self.physical_device,
            usage,
            properties,
        )?);

        Ok(())
    }

    pub unsafe fn free_temp_buffer(&mut self) {
        if self.temp_buffer.is_none() {
            return;
        }

        unsafe { self.temp_buffer.unwrap().free(&self.device()) };
    }

    pub fn get_uniform_buffers(&self, name: U) -> &Vec<BufferPair> {
        self.uniform_buffers.get(&name).unwrap()
    }

    pub unsafe fn create_buffer_descriptor_set<'a, UBO>(
        &self,
        binding: u32,
        name: U,
        descriptor_sets: &[DescriptorSet],
    ) -> Vec<WriteDescriptorSetBuilder<'a>> {
        let mut descriptors = vec![];

        for (i, buffer_pair) in self.get_uniform_buffers(name).iter().enumerate() {
            let info = DescriptorBufferInfo::builder()
                .buffer(buffer_pair.buffer)
                .offset(0)
                .range(size_of::<UBO>() as u64);

            let buffer_info = Box::new([info]);

            // WARNING: This probably never gets cleaned up I'm not really sure
            // TODO: Make sure this gets cleaned up if it doesn't automatically
            let buffer_info: &'a mut _ = Box::leak(buffer_info);

            descriptors.push(
                WriteDescriptorSet::builder()
                    .dst_set(descriptor_sets[i])
                    .dst_binding(binding)
                    .dst_array_element(0)
                    .descriptor_type(DescriptorType::UNIFORM_BUFFER)
                    .buffer_info(buffer_info),
            );
        }

        descriptors
    }

    pub unsafe fn allocate_new_standard_buffer<B>(
        &mut self,
        name: S,
        usage: BufferUsageFlags,
        properties: MemoryPropertyFlags,
    ) -> Result<()> {
        let buffer_pair = BufferPair::allocate::<B>(
            &self.instance(),
            &self.device(),
            self.physical_device,
            usage,
            properties,
        )?;

        let device = self.device();

        self.buffers
            .entry(name)
            .and_modify(|b| {
                unsafe { b.free(&device) }; // Ensure the buffer doesn't exist if it's getting replaced
                *b = buffer_pair;
            })
            .or_insert(buffer_pair);

        Ok(())
    }

    pub fn setup_uniform_buffer(&mut self, name: U) {
        self.uniform_buffers.entry(name).or_insert(vec![]);
    }

    pub unsafe fn allocate_new_uniform_buffer<B>(
        &mut self,
        name: U,
        usage: BufferUsageFlags,
        properties: MemoryPropertyFlags,
    ) -> Result<()> {
        let buffer_pair = BufferPair::allocate::<B>(
            &self.instance(),
            &self.device(),
            self.physical_device,
            usage,
            properties,
        )?;

        self.uniform_buffers
            .entry(name)
            .and_modify(|v| v.push(buffer_pair))
            .or_insert(vec![buffer_pair]);

        Ok(())
    }

    pub unsafe fn create_descriptor_set_layout<'a>(
        &mut self,
        additional_descriptors: Option<Vec<DescriptorSetLayoutBindingBuilder<'a>>>,
    ) -> Result<DescriptorSetLayout> {
        let mut bindings = additional_descriptors.unwrap_or(vec![]);

        for (i, _) in self.uniform_buffers.iter().enumerate() {
            info!("{i}");
            bindings.push(
                DescriptorSetLayoutBinding::builder()
                    .binding(i as u32)
                    .descriptor_type(DescriptorType::UNIFORM_BUFFER)
                    .descriptor_count(1)
                    .stage_flags(ShaderStageFlags::VERTEX),
            );
        }

        let info = DescriptorSetLayoutCreateInfo::builder().bindings(&bindings);
        unsafe { Ok(self.device().create_descriptor_set_layout(&info, None)?) }
    }

    pub unsafe fn free_uniform_buffers(&mut self, name: U) {
        let device = self.device();

        self.uniform_buffers.entry(name).and_modify(|v| {
            let _ = v
                .iter_mut()
                .map(|b| unsafe {
                    b.free(&device);
                })
                .collect::<Vec<_>>();
            v.clear();
        });
    }
}

#[derive(Default, Debug, Clone, Copy, PartialEq)]
pub struct BufferPair {
    pub buffer: Buffer,
    pub memory: DeviceMemory,
}

impl BufferPair {
    pub fn new(buffer: Buffer, memory: DeviceMemory) -> Self {
        Self { buffer, memory }
    }

    pub fn allocate<S>(
        instance: &Instance,
        device: &Device,
        physical_device: PhysicalDevice,
        usage: BufferUsageFlags,
        properties: MemoryPropertyFlags,
    ) -> Result<Self> {
        let buffer_info = BufferCreateInfo::builder()
            .size(size_of::<S>() as u64)
            .usage(usage)
            .sharing_mode(SharingMode::EXCLUSIVE);

        let buffer = unsafe { device.create_buffer(&buffer_info, None) }?;

        let requirements = unsafe { device.get_buffer_memory_requirements(buffer) };

        let memory_info = MemoryAllocateInfo::builder()
            .allocation_size(requirements.size)
            .memory_type_index(unsafe {
                get_memory_type_index(instance, physical_device, properties, requirements)
            }?);

        let buffer_memory = unsafe { device.allocate_memory(&memory_info, None) }?;

        (unsafe { device.bind_buffer_memory(buffer, buffer_memory, 0) })?;

        Ok(Self {
            buffer,
            memory: buffer_memory,
        })
    }

    pub fn split(&self) -> (Buffer, DeviceMemory) {
        (self.buffer, self.memory)
    }

    pub unsafe fn free(&mut self, device: &Device) {
        unsafe { device.destroy_buffer(self.buffer, None) };
        unsafe { device.free_memory(self.memory, None) };
    }
}
