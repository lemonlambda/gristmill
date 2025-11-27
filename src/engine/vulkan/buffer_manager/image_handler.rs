use std::fs::File;
use std::ptr::copy_nonoverlapping;

use anyhow::Result;
use vulkanalia::{Device, Instance};

use crate::engine::vulkan::VulkanApp;
use crate::engine::vulkan::VulkanData;
use crate::engine::vulkan::buffer_manager::buffer_operations::BufferAllocator;
use crate::engine::vulkan::buffer_manager::buffer_pair::create_buffer;
use crate::engine::vulkan::prelude::get_memory_type_index;
use vulkanalia::vk::*;

pub struct ImageData {
    pub pixels: Vec<u8>,
    pub width: u32,
    pub height: u32,
    pub size: u64,
}

impl ImageData {
    pub fn from_path(
        instance: &Instance,
        device: &Device,
        data: &mut VulkanData,
        path: &str,
    ) -> Result<Self> {
        let image = File::open(path)?;

        let decoder = png::Decoder::new(image);
        let mut reader = decoder.read_info()?;

        let mut pixels = vec![0; reader.info().raw_bytes()];
        reader.next_frame(&mut pixels)?;

        let size = reader.info().raw_bytes() as u64;
        let (width, height) = reader.info().size();

        Ok(Self {
            pixels,
            width,
            height,
            size,
        })
    }
}

pub struct Texture {
    image: Image,
    memory: DeviceMemory,
}

pub struct TextureAllocatorData<'a> {
    instance: &'a Instance,
    device: &'a Device,
    data: &'a mut VulkanData,
    image_data: ImageData,
}

impl Texture {
    unsafe fn create_image(
        instance: &Instance,
        device: &Device,
        data: &VulkanData,
        width: u32,
        height: u32,
        format: Format,
        tiling: ImageTiling,
        usage: ImageUsageFlags,
        properties: MemoryPropertyFlags,
    ) -> Result<(Image, DeviceMemory)> {
        // Image

        let info = ImageCreateInfo::builder()
            .image_type(ImageType::_2D)
            .extent(Extent3D {
                width,
                height,
                depth: 1,
            })
            .mip_levels(1)
            .array_layers(1)
            .format(format)
            .tiling(tiling)
            .initial_layout(ImageLayout::UNDEFINED)
            .usage(usage)
            .sharing_mode(SharingMode::EXCLUSIVE)
            .samples(SampleCountFlags::_1);

        let image = unsafe { device.create_image(&info, None) }?;

        // Memory

        let requirements = unsafe { device.get_image_memory_requirements(image) };

        let info = MemoryAllocateInfo::builder()
            .allocation_size(requirements.size)
            .memory_type_index(unsafe {
                get_memory_type_index(instance, data.physical_device, properties, requirements)
            }?);

        let image_memory = unsafe { device.allocate_memory(&info, None) }?;

        (unsafe { device.bind_image_memory(image, image_memory, 0) })?;

        Ok((image, image_memory))
    }
}

impl<'a> BufferAllocator for TextureAllocatorData<'a> {
    type Output = Texture;

    fn allocate_with_size(&mut self, size: u64) -> Result<Self::Output>
    where
        Self: Sized,
    {
        let (staging_buffer, staging_buffer_memory) = unsafe {
            create_buffer(
                self.instance,
                self.device,
                self.data,
                size,
                BufferUsageFlags::TRANSFER_SRC,
                MemoryPropertyFlags::HOST_COHERENT | MemoryPropertyFlags::HOST_VISIBLE,
            )
        }?;

        let memory = unsafe {
            self.device
                .map_memory(staging_buffer_memory, 0, size, MemoryMapFlags::empty())
        }?;

        unsafe {
            copy_nonoverlapping(
                self.image_data.pixels.as_ptr(),
                memory.cast(),
                self.image_data.pixels.len(),
            )
        };

        unsafe { self.device.unmap_memory(staging_buffer_memory) };

        let (texture_image, texture_image_memory) = unsafe {
            create_image(
                self.instance,
                self.device,
                self.data,
                self.image_data.width,
                self.image_data.height,
                Format::R8G8B8A8_SRGB,
                ImageTiling::OPTIMAL,
                ImageUsageFlags::SAMPLED | ImageUsageFlags::TRANSFER_DST,
                MemoryPropertyFlags::DEVICE_LOCAL,
            )
        }?;

        (unsafe {
            Self::transition_image_layout(
                self.device,
                self.data,
                self.data.texture_image,
                Format::R8G8B8A8_SRGB,
                ImageLayout::UNDEFINED,
                ImageLayout::TRANSFER_DST_OPTIMAL,
            )
        })?;

        (unsafe {
            Self::copy_buffer_to_image(
                self.device,
                self.data,
                self.staging_buffer,
                self.data.texture_image,
                self.image_data.width,
                self.image_data.height,
            )
        })?;

        (unsafe {
            Self::transition_image_layout(
                self.device,
                self.data,
                self.data.texture_image,
                Format::R8G8B8A8_SRGB,
                ImageLayout::TRANSFER_DST_OPTIMAL,
                ImageLayout::SHADER_READ_ONLY_OPTIMAL,
            )
        })?;

        // Cleanup

        unsafe { self.device.destroy_buffer(staging_buffer, None) };
        unsafe { self.device.free_memory(staging_buffer_memory, None) };
        Ok(Texture {
            image: texture_image,
            memory: texture_image_memory,
        })
    }
}
