use std::collections::HashMap;
use std::ffi::CString;
use std::fmt::{Debug, Display};
use std::ptr::copy_nonoverlapping;

use anyhow::{Result, anyhow};
use log::*;
use std::hash::Hash;
use vulkanalia::vk::*;
use vulkanalia::{Device, Instance};

use crate::engine::vulkan::VALIDATION_ENABLED;
use crate::engine::vulkan::shared_helpers::{copy_buffer, get_memory_type_index};

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

pub trait BufferManagerRequirements = Default + Eq + Hash + Debug + Display + Clone;

pub enum BufferManagerCopyType<S, U> {
    TempBuffer,
    StandardBuffer(S),
    UniformBuffers(U, usize),
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

pub enum BufferManagerDataType<'a, T, S, U> {
    Data(&'a [T]),
    TempBuffer {
        graphics_queue: Queue,
        command_pool: CommandPool,
    },
    StandardBuffer {
        name: S,
        graphics_queue: Queue,
        command_pool: CommandPool,
    },
    UniformBuffers {
        name: U,
        index: usize,
        graphics_queue: Queue,
        command_pool: CommandPool,
    },
}

impl<'a, T, S, U> PartialEq<BufferManagerCopyType<S, U>> for BufferManagerDataType<'a, T, S, U> {
    fn eq(&self, other: &BufferManagerCopyType<S, U>) -> bool {
        matches!(
            (self, other),
            (
                &BufferManagerDataType::TempBuffer { .. },
                &BufferManagerCopyType::TempBuffer
            ) | (
                &BufferManagerDataType::StandardBuffer { .. },
                &BufferManagerCopyType::StandardBuffer(_),
            ) | (
                &BufferManagerDataType::UniformBuffers { .. },
                &BufferManagerCopyType::UniformBuffers(_, _),
            )
        )
    }
}

pub enum AllocateBufferType<S, U> {
    Temp,
    Standard { name: S },
    Uniform { name: U },
}

impl<S, U> Display for AllocateBufferType<S, U> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            AllocateBufferType::Temp => f.write_str("AllocateBufferType::Temp"),
            AllocateBufferType::Standard { .. } => f.write_str("AllocateBufferType::Standard"),
            AllocateBufferType::Uniform { .. } => f.write_str("AllocateBufferType::Uniform"),
        }
    }
}

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

    /// Internal function to skip writing out unwrap
    fn instance(&self) -> Instance {
        self.instance.clone().unwrap()
    }
    /// Internal function to skip writing out unwrap
    fn device(&self) -> Device {
        self.device.clone().unwrap()
    }

    pub unsafe fn create_descriptor_pool(
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

    pub unsafe fn copy_data_to_buffer<'a, T>(
        &mut self,
        data: BufferManagerDataType<'a, T, S, U>,
        buffer_type: BufferManagerCopyType<S, U>,
    ) -> Result<()> {
        unsafe { self.copy_data_to_buffer_with_size(data, buffer_type, size_of::<T>() as u64) }
    }

    pub unsafe fn copy_data_to_buffer_with_size<'a, T>(
        &mut self,
        data: BufferManagerDataType<'a, T, S, U>,
        buffer_type: BufferManagerCopyType<S, U>,
        size: u64,
    ) -> Result<()> {
        if data == buffer_type {
            return Err(anyhow!("`data` and `buffer_type` are the same."));
        }

        let buffer_pair = match buffer_type {
            BufferManagerCopyType::TempBuffer => self
                .temp_buffer
                .ok_or(anyhow!("Temp buffer not allocated."))?,
            BufferManagerCopyType::StandardBuffer(ref name) => *self
                .buffers
                .get(name)
                .ok_or(anyhow!("{name:?} buffer not allocated."))?,
            BufferManagerCopyType::UniformBuffers(ref name, i) => *self
                .uniform_buffers
                .get(name)
                .ok_or(anyhow!("{name:?} buffer not allocated."))?
                .get(i)
                .ok_or(anyhow!("{i} index doesn't exist in {name:?}."))?,
        };

        match data {
            BufferManagerDataType::Data(value) => {
                unsafe {
                    let destination = self.device().map_memory(
                        buffer_pair.memory,
                        0,
                        size,
                        MemoryMapFlags::empty(),
                    )?;

                    copy_nonoverlapping(value.as_ptr(), destination.cast(), value.len());
                };
            }
            BufferManagerDataType::TempBuffer {
                graphics_queue,
                command_pool,
            } => unsafe {
                let source = self
                    .temp_buffer
                    .ok_or(anyhow!("Temp buffer not allocated."))?;

                self.device().unmap_memory(source.memory);

                copy_buffer(
                    &self.device(),
                    graphics_queue,
                    command_pool,
                    source.buffer,
                    buffer_pair.buffer,
                    size,
                )?;
            },
            BufferManagerDataType::StandardBuffer {
                ref name,
                graphics_queue,
                command_pool,
            } => unsafe {
                let source = self
                    .buffers
                    .get(name)
                    .ok_or(anyhow!("{name:?} is not allocated."))?;

                self.device().unmap_memory(source.memory);

                copy_buffer(
                    &self.device(),
                    graphics_queue,
                    command_pool,
                    source.buffer,
                    buffer_pair.buffer,
                    size,
                )?;
            },
            BufferManagerDataType::UniformBuffers {
                ref name,
                index,
                graphics_queue,
                command_pool,
            } => unsafe {
                let source = self
                    .uniform_buffers
                    .get(name)
                    .ok_or(anyhow!("{name:?} is not allocated."))?
                    .get(index)
                    .ok_or(anyhow!("{index} doesn't exist in {name:?}."))?;

                self.device().unmap_memory(source.memory);

                copy_buffer(
                    &self.device(),
                    graphics_queue,
                    command_pool,
                    source.buffer,
                    buffer_pair.buffer,
                    size,
                )?;
            },
        };

        Ok(())
    }

    pub unsafe fn allocate_buffer_with_size(
        &mut self,
        buffer_type: AllocateBufferType<S, U>,
        usage: BufferUsageFlags,
        properties: MemoryPropertyFlags,
        size: u64,
    ) -> Result<()> {
        debug!("Allocating a buffer named: {}", buffer_type);
        let buffer_pair = BufferPair::allocate_with_size(
            &self.instance(),
            &self.device(),
            self.physical_device,
            usage,
            properties,
            size,
        )?;

        match buffer_type {
            AllocateBufferType::Temp => {
                unsafe { self.free_temp_buffer() }; // Make sure no temp buffer exists already
                self.temp_buffer = Some(buffer_pair);
            }
            AllocateBufferType::Standard { name } => {
                let device = self.device();

                self.buffers
                    .entry(name)
                    .and_modify(|b| {
                        unsafe { b.free(&device) }; // Ensure the buffer doesn't exist if it's getting replaced
                        *b = buffer_pair;
                    })
                    .or_insert(buffer_pair);
            }
            AllocateBufferType::Uniform { name } => {
                self.uniform_buffers
                    .entry(name)
                    .and_modify(|v| v.push(buffer_pair))
                    .or_insert(vec![buffer_pair]);
            }
        };

        // Debug info for validation
        if VALIDATION_ENABLED {
            unsafe {
                self.instance().set_debug_utils_object_name_ext(
                    self.device().handle(),
                    &DebugUtilsObjectNameInfoEXT {
                        s_type: StructureType::DEBUG_UTILS_OBJECT_NAME_INFO_EXT,
                        next: std::ptr::null(),
                        object_type: ObjectType::BUFFER,
                        object_handle: buffer_pair.buffer.as_raw(),
                        object_name: CString::new("TempBuffer").unwrap().as_ptr(),
                    },
                )?
            };
        }

        Ok(())
    }

    pub unsafe fn allocate_buffer<B>(
        &mut self,
        buffer_type: AllocateBufferType<S, U>,
        usage: BufferUsageFlags,
        properties: MemoryPropertyFlags,
    ) -> Result<()> {
        unsafe {
            self.allocate_buffer_with_size(buffer_type, usage, properties, size_of::<B>() as u64)
        }
    }

    pub fn get_uniform_buffers(&self, name: U) -> &Vec<BufferPair> {
        self.uniform_buffers.get(&name).unwrap()
    }

    pub fn get_standard_buffer(&mut self, name: S) -> &BufferPair {
        self.buffers.get(&name).unwrap()
    }

    pub fn setup_uniform_buffer(&mut self, name: U) {
        self.uniform_buffers.entry(name).or_insert(vec![]);
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

    pub unsafe fn create_descriptor_set_layout<'a>(
        &mut self,
        additional_descriptors: Option<Vec<DescriptorSetLayoutBindingBuilder<'a>>>,
    ) -> Result<DescriptorSetLayout> {
        let mut bindings = additional_descriptors.unwrap_or(vec![]);

        for (i, _) in self.uniform_buffers.iter().enumerate() {
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

    pub unsafe fn free_temp_buffer(&mut self) {
        if self.temp_buffer.is_none() {
            return;
        }

        unsafe { self.temp_buffer.unwrap().free(&self.device()) };

        self.temp_buffer = None;
    }

    pub unsafe fn free_standard_buffer(&mut self, name: S) {
        let device = self.device();

        self.buffers
            .entry(name)
            .and_modify(|b| unsafe { b.free(&device) });
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

    pub fn allocate_with_size(
        instance: &Instance,
        device: &Device,
        physical_device: PhysicalDevice,
        usage: BufferUsageFlags,
        properties: MemoryPropertyFlags,
        size: u64,
    ) -> Result<Self> {
        debug!("Allocating a buffer");
        let buffer_info = BufferCreateInfo::builder()
            .size(size)
            .usage(usage)
            .sharing_mode(SharingMode::EXCLUSIVE);

        let buffer = unsafe { device.create_buffer(&buffer_info, None) }?;

        let requirements = unsafe { device.get_buffer_memory_requirements(buffer) };
        debug!(
            "Buffer Requirement Size: {}, with input size {}",
            requirements.size, size
        );

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

    pub fn allocate<S>(
        instance: &Instance,
        device: &Device,
        physical_device: PhysicalDevice,
        usage: BufferUsageFlags,
        properties: MemoryPropertyFlags,
    ) -> Result<Self> {
        Self::allocate_with_size(
            instance,
            device,
            physical_device,
            usage,
            properties,
            size_of::<S>() as u64,
        )
    }

    pub fn split(&self) -> (Buffer, DeviceMemory) {
        (self.buffer, self.memory)
    }

    pub unsafe fn free(&mut self, device: &Device) {
        unsafe { device.destroy_buffer(self.buffer, None) };
        unsafe { device.free_memory(self.memory, None) };
    }
}
