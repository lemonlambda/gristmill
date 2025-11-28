use std::fmt::Display;
use std::fs::File;
use std::ptr::copy_nonoverlapping;

use anyhow::{Result, anyhow};
use vulkanalia::{Device, Instance};

use super::super::prelude::*;
use crate::engine::vulkan::VulkanApp;
use crate::engine::vulkan::VulkanData;
use crate::engine::vulkan::buffer_manager::buffer_operations::BufferAllocator;
use crate::engine::vulkan::buffer_manager::buffer_operations::BufferOperations;
use crate::engine::vulkan::buffer_manager::buffer_pair::create_buffer;
use crate::engine::vulkan::prelude::begin_single_time_commands;
use crate::engine::vulkan::prelude::end_single_time_commands;
use crate::engine::vulkan::prelude::get_memory_type_index;
use vulkanalia::vk::*;

#[derive(Default, Debug, Clone, Copy, Eq, PartialEq, Hash)]
pub enum TextureName {
    #[default]
    Bird,
    Depth,
}

impl Display for TextureName {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            TextureName::Bird => f.write_str("TextureName::Bird"),
            TextureName::Depth => f.write_str("TextureName::Depth"),
        }
    }
}

#[derive(Default, Debug, Clone, Copy, Eq, PartialEq, Hash)]
pub enum TextureGroupName {
    #[default]
    Empty,
}

impl Display for TextureGroupName {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            TextureGroupName::Empty => f.write_str("TextureGroupName::Empty"),
        }
    }
}

pub struct ImageData {
    pub pixels: Option<Vec<u8>>,
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
            pixels: Some(pixels),
            width,
            height,
            size,
        })
    }
}

#[derive(Default, Debug, Clone, Copy, Eq, PartialEq, Hash)]
pub struct Texture {
    pub image: Image,
    pub memory: DeviceMemory,
    pub image_view: ImageView,
}

impl BufferOperations for Texture {
    type DropData<'a> = Device;
    type BufferType = Image;

    fn get_buffer(&self) -> Self::BufferType {
        self.image.clone()
    }

    fn get_memory(&self) -> DeviceMemory {
        self.memory.clone()
    }

    unsafe fn free<'a>(&mut self, drop_data: Self::DropData<'a>) {
        unsafe {
            drop_data.destroy_image_view(self.image_view, None);
            drop_data.destroy_image(self.image, None);
            drop_data.free_memory(self.memory, None);
        }
    }
}

pub struct TextureAllocatorData<'a> {
    pub instance: &'a Instance,
    pub device: &'a Device,
    pub data: &'a mut VulkanData,
    pub image_data: ImageData,
    pub format: Format,
    pub tiling: ImageTiling,
    pub usage: ImageUsageFlags,
    pub properties: MemoryPropertyFlags,
    pub image_aspects: ImageAspectFlags,
    pub transition_layout: ImageLayout,
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

impl<'a> TextureAllocatorData<'a> {
    unsafe fn transition_image_layout(
        device: &Device,
        data: &VulkanData,
        image: Image,
        format: Format,
        old_layout: ImageLayout,
        new_layout: ImageLayout,
    ) -> Result<()> {
        let (src_access_mask, dst_access_mask, src_stage_mask, dst_stage_mask) =
            match (old_layout, new_layout) {
                (ImageLayout::UNDEFINED, ImageLayout::DEPTH_STENCIL_ATTACHMENT_OPTIMAL) => (
                    AccessFlags::empty(),
                    AccessFlags::DEPTH_STENCIL_ATTACHMENT_READ
                        | AccessFlags::DEPTH_STENCIL_ATTACHMENT_WRITE,
                    PipelineStageFlags::TOP_OF_PIPE,
                    PipelineStageFlags::EARLY_FRAGMENT_TESTS,
                ),
                (ImageLayout::UNDEFINED, ImageLayout::TRANSFER_DST_OPTIMAL) => (
                    AccessFlags::empty(),
                    AccessFlags::TRANSFER_WRITE,
                    PipelineStageFlags::TOP_OF_PIPE,
                    PipelineStageFlags::TRANSFER,
                ),
                (ImageLayout::TRANSFER_DST_OPTIMAL, ImageLayout::SHADER_READ_ONLY_OPTIMAL) => (
                    AccessFlags::TRANSFER_WRITE,
                    AccessFlags::SHADER_READ,
                    PipelineStageFlags::TRANSFER,
                    PipelineStageFlags::FRAGMENT_SHADER,
                ),
                _ => return Err(anyhow!("Unsupported image layout transition!")),
            };

        let command_buffer = unsafe { begin_single_time_commands(device, data.command_pool) }?;

        let aspect_mask = if new_layout == ImageLayout::DEPTH_STENCIL_ATTACHMENT_OPTIMAL {
            match format {
                Format::D32_SFLOAT_S8_UINT | Format::D24_UNORM_S8_UINT => {
                    ImageAspectFlags::DEPTH | ImageAspectFlags::STENCIL
                }
                _ => ImageAspectFlags::DEPTH,
            }
        } else {
            ImageAspectFlags::COLOR
        };

        let subresource = ImageSubresourceRange::builder()
            .aspect_mask(aspect_mask)
            .base_mip_level(0)
            .level_count(1)
            .base_array_layer(0)
            .layer_count(1);

        let barrier = ImageMemoryBarrier::builder()
            .old_layout(old_layout)
            .new_layout(new_layout)
            .src_queue_family_index(QUEUE_FAMILY_IGNORED)
            .dst_queue_family_index(QUEUE_FAMILY_IGNORED)
            .image(image)
            .subresource_range(subresource)
            .src_access_mask(src_access_mask)
            .dst_access_mask(dst_access_mask);

        unsafe {
            device.cmd_pipeline_barrier(
                command_buffer,
                src_stage_mask,
                dst_stage_mask,
                DependencyFlags::empty(),
                &[] as &[MemoryBarrier],
                &[] as &[BufferMemoryBarrier],
                &[barrier],
            )
        };

        (unsafe {
            end_single_time_commands(
                device,
                data.graphics_queue,
                data.command_pool,
                command_buffer,
            )
        })?;

        Ok(())
    }

    unsafe fn copy_buffer_to_image(
        device: &Device,
        data: &VulkanData,
        buffer: Buffer,
        image: Image,
        width: u32,
        height: u32,
    ) -> Result<()> {
        let command_buffer = unsafe { begin_single_time_commands(device, data.command_pool) }?;

        let subresource = ImageSubresourceLayers::builder()
            .aspect_mask(ImageAspectFlags::COLOR)
            .mip_level(0)
            .base_array_layer(0)
            .layer_count(1);

        let region = BufferImageCopy::builder()
            .buffer_offset(0)
            .buffer_row_length(0)
            .buffer_image_height(0)
            .image_subresource(subresource)
            .image_offset(Offset3D { x: 0, y: 0, z: 0 })
            .image_extent(Extent3D {
                width,
                height,
                depth: 1,
            });

        unsafe {
            device.cmd_copy_buffer_to_image(
                command_buffer,
                buffer,
                image,
                ImageLayout::TRANSFER_DST_OPTIMAL,
                &[region],
            )
        };

        (unsafe {
            end_single_time_commands(
                device,
                data.graphics_queue,
                data.command_pool,
                command_buffer,
            )
        })?;

        Ok(())
    }

    unsafe fn create_image_view(
        device: &Device,
        image: Image,
        format: Format,
        aspects: ImageAspectFlags,
    ) -> Result<ImageView> {
        let subresource_range = ImageSubresourceRange::builder()
            .aspect_mask(aspects)
            .base_mip_level(0)
            .level_count(1)
            .base_array_layer(0)
            .layer_count(1);

        let info = ImageViewCreateInfo::builder()
            .image(image)
            .view_type(ImageViewType::_2D)
            .format(format)
            .subresource_range(subresource_range);

        Ok(unsafe { device.create_image_view(&info, None) }?)
    }
}

impl<'a> BufferAllocator for TextureAllocatorData<'a> {
    type Output = Texture;

    fn allocate_with_size(&mut self, size: u64) -> Result<Self::Output>
    where
        Self: Sized,
    {
        let (texture_image, texture_image_memory) = unsafe {
            Texture::create_image(
                self.instance,
                self.device,
                self.data,
                self.image_data.width,
                self.image_data.height,
                self.format,
                self.tiling,
                self.usage,
                self.properties,
            )
        }?;

        if let Some(pixels) = self.image_data.pixels.clone() {
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

            unsafe { copy_nonoverlapping(pixels.as_ptr(), memory.cast(), size as usize) };

            unsafe { self.device.unmap_memory(staging_buffer_memory) };

            (unsafe {
                Self::transition_image_layout(
                    self.device,
                    self.data,
                    texture_image,
                    self.format,
                    ImageLayout::UNDEFINED,
                    ImageLayout::TRANSFER_DST_OPTIMAL,
                )
            })?;

            (unsafe {
                Self::copy_buffer_to_image(
                    self.device,
                    self.data,
                    staging_buffer,
                    texture_image,
                    self.image_data.width,
                    self.image_data.height,
                )
            })?;
            (unsafe {
                Self::transition_image_layout(
                    self.device,
                    self.data,
                    texture_image,
                    self.format,
                    ImageLayout::TRANSFER_DST_OPTIMAL,
                    self.transition_layout,
                )
            })?;

            unsafe { self.device.destroy_buffer(staging_buffer, None) };
            unsafe { self.device.free_memory(staging_buffer_memory, None) };
        } else {
            (unsafe {
                Self::transition_image_layout(
                    self.device,
                    self.data,
                    texture_image,
                    self.format,
                    ImageLayout::UNDEFINED,
                    self.transition_layout,
                )
            })?;
        }

        // Cleanup

        Ok(Texture {
            image: texture_image,
            memory: texture_image_memory,
            image_view: unsafe {
                Self::create_image_view(
                    self.device,
                    texture_image,
                    self.format,
                    self.image_aspects,
                )?
            },
        })
    }
}
