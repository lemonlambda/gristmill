use std::collections::HashMap;
use std::ffi::CString;
use std::fmt::{Debug, Display};
use std::marker::PhantomData;
use std::ptr::copy_nonoverlapping;

use anyhow::{Result, anyhow};
use log::*;
use std::hash::Hash;
use vulkanalia::vk::*;
use vulkanalia::{Device, Instance};

use crate::engine::vulkan::VALIDATION_ENABLED;
use crate::engine::vulkan::buffer_manager::buffer_operations::{
    BufferAllocator, BufferOperations, SupportsCopying,
};
use crate::engine::vulkan::shared_helpers::{copy_buffer, get_memory_type_index};

pub mod buffer_operations;
pub mod buffer_pair;
// pub mod image_handler;

pub trait BufferManagerRequirements = Default + Debug + Display + Hash + Eq + PartialEq;

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

pub enum BufferManagerCopyType<S, U> {
    TempBuffer,
    StandardBuffer(S),
    UniformBuffers(U, usize),
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

#[derive(Debug, Clone)]
pub struct BufferManager<
    Buffer: BufferOperations + Default,
    Standard: BufferManagerRequirements,
    Uniform: BufferManagerRequirements,
> {
    pub instance: Option<Instance>,
    pub device: Option<Device>,
    pub physical_device: PhysicalDevice,
    pub temp_buffer: Option<Buffer>,
    pub buffers: HashMap<Standard, Buffer>,
    pub uniform_buffers: HashMap<Uniform, Vec<Buffer>>,
    pub drop_data: Option<Buffer::DropData<'static>>,
}

impl<
    Buffer: BufferOperations + Default,
    Standard: BufferManagerRequirements,
    Uniform: BufferManagerRequirements,
> Default for BufferManager<Buffer, Standard, Uniform>
{
    fn default() -> Self {
        Self {
            instance: None,
            device: None,
            physical_device: PhysicalDevice::default(),
            drop_data: None,
            temp_buffer: None,
            buffers: HashMap::new(),
            uniform_buffers: HashMap::new(),
        }
    }
}

impl<
    Buffer: BufferOperations + Default,
    Standard: BufferManagerRequirements,
    Uniform: BufferManagerRequirements,
> BufferManager<Buffer, Standard, Uniform>
{
    pub fn new(instance: Instance, device: Device, drop_data: Buffer::DropData<'static>) -> Self {
        Self {
            instance: Some(instance),
            device: Some(device.clone()),
            physical_device: device.physical_device(),
            drop_data: Some(drop_data),
            temp_buffer: None,
            buffers: HashMap::new(),
            uniform_buffers: HashMap::new(),
        }
    }

    pub fn instance(&self) -> Instance {
        self.instance.clone().unwrap()
    }

    pub fn device(&self) -> Device {
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
        data: BufferManagerDataType<'a, T, Standard, Uniform>,
        buffer_type: BufferManagerCopyType<Standard, Uniform>,
    ) -> Result<()>
    where
        Buffer: SupportsCopying + Clone,
    {
        unsafe { self.copy_data_to_buffer_with_size(data, buffer_type, size_of::<T>() as u64) }
    }

    pub unsafe fn copy_data_to_buffer_with_size<'a, T>(
        &mut self,
        data: BufferManagerDataType<'a, T, Standard, Uniform>,
        buffer_type: BufferManagerCopyType<Standard, Uniform>,
        size: u64,
    ) -> Result<()>
    where
        Buffer: SupportsCopying + Clone,
    {
        if data == buffer_type {
            return Err(anyhow!("`data` and `buffer_type` are the same."));
        }

        let buffer_pair = match buffer_type {
            BufferManagerCopyType::TempBuffer => self
                .temp_buffer
                .clone()
                .ok_or(anyhow!("Temp buffer not allocated."))?
                .clone(),
            BufferManagerCopyType::StandardBuffer(ref name) => self
                .buffers
                .get(name)
                .ok_or(anyhow!("{name:?} buffer not allocated."))?
                .clone(),
            BufferManagerCopyType::UniformBuffers(ref name, i) => self
                .uniform_buffers
                .get(name)
                .ok_or(anyhow!("{name:?} buffer not allocated."))?
                .get(i)
                .ok_or(anyhow!("{i} index doesn't exist in {name:?}."))?
                .clone(),
        };

        let device = self.device().clone();

        match data {
            BufferManagerDataType::Data(value) => {
                unsafe {
                    let destination = self.device().map_memory(
                        buffer_pair.get_memory(),
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
            } => {
                let source = self
                    .temp_buffer
                    .as_mut()
                    .ok_or(anyhow!("Temp buffer not allocated."))?;

                source.copy(buffer_pair, graphics_queue, command_pool, device, size)?;
            }
            BufferManagerDataType::StandardBuffer {
                ref name,
                graphics_queue,
                command_pool,
            } => {
                let source = self
                    .buffers
                    .get_mut(name)
                    .ok_or(anyhow!("{name:?} is not allocated."))?;

                source.copy(buffer_pair, graphics_queue, command_pool, device, size)?;
            }
            BufferManagerDataType::UniformBuffers {
                ref name,
                index,
                graphics_queue,
                command_pool,
            } => {
                let source = self
                    .uniform_buffers
                    .get_mut(name)
                    .ok_or(anyhow!("{name:?} is not allocated."))?
                    .get_mut(index)
                    .ok_or(anyhow!("{index} doesn't exist in {name:?}."))?;

                source.copy(buffer_pair, graphics_queue, command_pool, device, size)?;
            }
        };

        Ok(())
    }

    pub unsafe fn allocate_buffer_with_size<BufferCreator: BufferAllocator<Output = Buffer>>(
        &mut self,
        buffer_type: AllocateBufferType<Standard, Uniform>,
        mut needed_data: BufferCreator,
        size: u64,
    ) -> Result<()> {
        debug!("Allocating a buffer named: {}", buffer_type);
        let buffer = needed_data.allocate_with_size(size)?;

        match buffer_type {
            AllocateBufferType::Temp => {
                unsafe { self.free_temp_buffer() }; // Make sure no temp buffer exists already
                self.temp_buffer = Some(buffer);
            }
            AllocateBufferType::Standard { name } => {
                if let Some(b) = self.buffers.get_mut(&name) {
                    unsafe {
                        b.free(self.drop_data.clone().unwrap());
                    }
                    *b = buffer;
                } else {
                    self.buffers.insert(name, buffer);
                }
            }
            AllocateBufferType::Uniform { name } => {
                if let Some(v) = self.uniform_buffers.get_mut(&name) {
                    v.push(buffer);
                } else {
                    self.uniform_buffers.insert(name, vec![buffer]);
                }
            }
        };

        // TODO: Implement a debug function on buffers
        // // Debug info for validation
        // if VALIDATION_ENABLED {
        //     unsafe {
        //         self.instance().set_debug_utils_object_name_ext(
        //             self.device().handle(),
        //             &DebugUtilsObjectNameInfoEXT {
        //                 s_type: StructureType::DEBUG_UTILS_OBJECT_NAME_INFO_EXT,
        //                 next: std::ptr::null(),
        //                 object_type: ObjectType::BUFFER,
        //                 object_handle: buffer_pair.get_buffer().as_raw(),
        //                 object_name: CString::new("TempBuffer").unwrap().as_ptr(),
        //             },
        //         )?
        //     };
        // }

        Ok(())
    }

    pub unsafe fn allocate_buffer<BufferCreator: BufferAllocator<Output = Buffer>, Size>(
        &mut self,
        buffer_type: AllocateBufferType<Standard, Uniform>,
        needed_data: BufferCreator,
    ) -> Result<()> {
        unsafe {
            self.allocate_buffer_with_size(buffer_type, needed_data, size_of::<Size>() as u64)
        }
    }

    pub fn get_uniform_buffers(&self, name: Uniform) -> &Vec<Buffer> {
        self.uniform_buffers.get(&name).unwrap()
    }

    pub fn get_standard_buffer(&mut self, name: Standard) -> &Buffer {
        self.buffers.get(&name).unwrap()
    }

    pub fn setup_uniform_buffer(&mut self, name: Uniform) {
        self.uniform_buffers.entry(name).or_insert(vec![]);
    }

    pub unsafe fn create_buffer_descriptor_set<'a, UBOSize>(
        &self,
        binding: u32,
        name: Uniform,
        descriptor_sets: &[DescriptorSet],
    ) -> Vec<WriteDescriptorSetBuilder<'a>>
    where
        Buffer: Into<vulkanalia::vk::Buffer> + Clone,
    {
        let mut descriptors = vec![];

        for (i, buffer_pair) in self.get_uniform_buffers(name).iter().enumerate() {
            let info = DescriptorBufferInfo::builder()
                .buffer(buffer_pair.clone().into())
                .offset(0)
                .range(size_of::<UBOSize>() as u64);

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

        unsafe {
            self.temp_buffer
                .as_mut()
                .unwrap()
                .free(self.drop_data.clone().unwrap())
        };

        self.temp_buffer = None;
    }

    pub unsafe fn free_standard_buffer(&mut self, name: Standard) {
        self.buffers
            .entry(name)
            .and_modify(|b| unsafe { b.free(self.drop_data.clone().unwrap()) });
    }

    pub unsafe fn free_uniform_buffers(&mut self, name: Uniform) {
        self.uniform_buffers.entry(name).and_modify(|v| {
            let _ = v
                .iter_mut()
                .map(|b| unsafe {
                    b.free(self.drop_data.clone().unwrap());
                })
                .collect::<Vec<_>>();
            v.clear();
        });
    }
}
