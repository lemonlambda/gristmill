use anyhow::{Result, anyhow};
use cgmath::{Deg, Matrix};
use log::*;
use std::{
    collections::HashSet,
    ffi::{CStr, c_void},
    fs::File,
    ptr::copy_nonoverlapping,
    time::Instant,
};
use thiserror::Error;
use vulkanalia::{
    Device, Entry, Instance, Version,
    bytecode::Bytecode,
    loader::{LIBRARY, LibloadingLoader},
    vk::*,
    window::{create_surface, get_required_instance_extensions},
};
use winit::window::Window;

use crate::engine::{
    gui::GuiApp,
    vertex::{INDICES, Mat4, SporadicBufferObject, UniformBufferObject, VERTICES, Vertex},
    vulkan::{
        buffer_manager::{
            AllocateBufferType, BufferManager, BufferManagerCopyType, BufferManagerDataType,
            StandardBufferMaps, UniformBufferMaps,
        },
        shared_helpers::{
            begin_single_time_commands, end_single_time_commands, get_memory_type_index,
        },
    },
};

pub mod buffer_manager;
pub mod shared_helpers;

const PORTABILITY_MACOS_VERSION: Version = Version::new(1, 3, 216);

const VALIDATION_ENABLED: bool = cfg!(debug_assertions);
const VALIDATION_LAYER: ExtensionName = ExtensionName::from_bytes(b"VK_LAYER_KHRONOS_validation");

const DEVICE_EXTENSIONS: &[ExtensionName] = &[
    KHR_SWAPCHAIN_EXTENSION.name,
    KHR_TIMELINE_SEMAPHORE_EXTENSION.name,
];

const MAX_FRAMES_IN_FLIGHT: usize = 2;

extern "system" fn debug_callback(
    severity: DebugUtilsMessageSeverityFlagsEXT,
    type_: DebugUtilsMessageTypeFlagsEXT,
    data: *const DebugUtilsMessengerCallbackDataEXT,
    _: *mut c_void,
) -> Bool32 {
    let data = unsafe { *data };
    let message = unsafe { CStr::from_ptr(data.message) }.to_string_lossy();

    if severity >= DebugUtilsMessageSeverityFlagsEXT::ERROR {
        error!("({:?}) {}", type_, message);
    } else if severity >= DebugUtilsMessageSeverityFlagsEXT::WARNING {
        warn!("({:?}) {}", type_, message);
    } else if severity >= DebugUtilsMessageSeverityFlagsEXT::INFO {
        debug!("({:?}) {}", type_, message);
    } else {
        trace!("({:?}) {}", type_, message);
    }

    FALSE
}

#[derive(Debug, Error)]
#[error("{0}")]
pub struct SuitabilityError(pub &'static str);

pub struct VulkanApp {
    pub entry: Entry,
    pub instance: Instance,
    pub device: Device,
    pub data: VulkanData,
    pub frame: usize,
    pub resized: bool,
    start: Instant,
    pub camera_position: [f32; 2],
    pub window: Window,
    pub gui: GuiApp,
}

#[derive(Clone, Debug, Default)]
pub struct VulkanData {
    pub messenger: DebugUtilsMessengerEXT,
    pub physical_device: PhysicalDevice,
    pub graphics_queue: Queue,
    pub present_queue: Queue,
    pub surface: SurfaceKHR,
    pub swapchain: SwapchainKHR,
    pub swapchain_format: Format,
    pub swapchain_extent: Extent2D,
    pub swapchain_images: Vec<Image>,
    pub swapchain_image_views: Vec<ImageView>,
    pub descriptor_set_layout: DescriptorSetLayout,
    pub pipeline_layout: PipelineLayout,
    pub render_pass: RenderPass,
    pub pipeline: Pipeline,
    pub framebuffers: Vec<Framebuffer>,
    pub command_pool: CommandPool,
    pub command_buffers: Vec<CommandBuffer>,
    pub image_available_semaphore: Vec<Semaphore>,
    pub render_finished_semaphore: Vec<Semaphore>,
    pub in_flight_fences: Vec<Fence>,
    pub images_in_flight: Vec<Fence>,
    pub descriptor_pool: DescriptorPool,
    pub descriptor_sets: Vec<DescriptorSet>,
    pub swapchain_min_image_count: u32,
    pub texture_image: Image,
    pub texture_image_memory: DeviceMemory,
    pub texture_image_view: ImageView,
    pub texture_sampler: Sampler,
    pub depth_image: Image,
    pub depth_image_memory: DeviceMemory,
    pub depth_image_view: ImageView,
    pub buffer_manager: BufferManager<StandardBufferMaps, UniformBufferMaps>,
}

impl VulkanApp {
    pub unsafe fn create(window: Window) -> Result<Self> {
        unsafe {
            let loader = LibloadingLoader::new(LIBRARY)?;
            let entry = Entry::new(loader).map_err(|b| anyhow!("{}", b))?;
            let mut data = VulkanData::default();
            let instance = Self::create_instance(&window, &entry, &mut data)?;
            data.surface = create_surface(&instance, &window, &window)?;
            Self::pick_physical_device(&instance, &mut data)?;
            let device = Self::create_logical_device(&entry, &instance, &mut data)?;
            unsafe {
                data.buffer_manager = BufferManager::<StandardBufferMaps, UniformBufferMaps>::new(
                    instance.clone(),
                    device.clone(),
                    data.physical_device,
                );
                Self::create_swapchain(&window, &instance, &device, &mut data)?;
                Self::create_swapchain_image_views(&device, &mut data)?;
                Self::create_render_pass(&instance, &device, &mut data)?;
                Self::create_descriptor_set_layout(&mut data)?;
                Self::create_pipeline(&device, &mut data)?;
                Self::create_command_pool(&instance, &device, &mut data)?;
                Self::create_depth_objects(&instance, &device, &mut data)?;
                Self::create_framebuffers(&device, &mut data)?;
                Self::create_texture_image(&instance, &device, &mut data)?;
                Self::create_texture_image_view(&device, &mut data)?;
                Self::create_vertex_buffer(&mut data)?;
                Self::create_texture_sampler(&device, &mut data)?;
                Self::create_index_buffer(&mut data)?;
                Self::create_uniform_buffers(&mut data)?;
                Self::create_descriptor_pool(&mut data)?;
                Self::create_descriptor_sets(&device, &mut data)?;
                Self::create_command_buffers(&device, &mut data)?;
                Self::create_sync_objects(&device, &mut data)?;
                info!("Woo created everything, hard work ain't it?");
            }

            let gui = GuiApp::new(&window);

            Ok(Self {
                entry,
                instance,
                device,
                data,
                frame: 0,
                resized: false,
                start: Instant::now(),
                camera_position: [0.0, 0.0],
                window,
                gui,
            })
        }
    }

    pub unsafe fn create_imgui_buffers(
        data: &mut VulkanData,
        window: &Window,
        gui: &mut GuiApp,
    ) -> Result<()> {
        let vertices = gui.render(window)?;
        for (i, (vertices, indices)) in vertices.into_iter().enumerate() {
            unsafe {
                Self::create_imgui_vertex_buffer(data, vertices, i)?;
                Self::create_imgui_index_buffer(data, indices, i)?;
            }
        }

        Ok(())
    }

    pub unsafe fn create_imgui_vertex_buffer(
        data: &mut VulkanData,
        vertices: Vec<Vertex>,
        idx: usize,
    ) -> Result<()> {
        unsafe {
            let vertex_buffer_size = (size_of::<Vertex>()  * vertices.len()) as u64;

            data.buffer_manager.allocate_buffer_with_size(
                AllocateBufferType::Temp,
                BufferUsageFlags::TRANSFER_SRC,
                MemoryPropertyFlags::HOST_COHERENT | MemoryPropertyFlags::HOST_VISIBLE,
                vertex_buffer_size
            )?;

            data.buffer_manager.copy_data_to_buffer(
                BufferManagerDataType::Data(&VERTICES),
                BufferManagerCopyType::TempBuffer,
            )?;

            data.buffer_manager.allocate_buffer_with_size(
                AllocateBufferType::Standard {
                    name: StandardBufferMaps::Vertices,
                },
                BufferUsageFlags::VERTEX_BUFFER | BufferUsageFlags::TRANSFER_DST,
                MemoryPropertyFlags::DEVICE_LOCAL,
                vertex_buffer_size
            )?;

            data.buffer_manager
                .copy_data_to_buffer::<Vertex>(
                    BufferManagerDataType::TempBuffer {
                        graphics_queue: data.graphics_queue,
                        command_pool: data.command_pool,
                    },
                    BufferManagerCopyType::StandardBuffer(StandardBufferMaps::Vertices),
                )?;

            data.buffer_manager.free_temp_buffer()
        };

        Ok(())
    }

    pub unsafe fn create_imgui_index_buffer(
        data: &mut VulkanData,
        indices: Vec<u16>,
        idx: usize,
    ) -> Result<()> {
        unsafe {
            type IndexBufferSize = [u16; INDICES.len()];

            data.buffer_manager
                .allocate_buffer::<IndexBufferSize>(
                    AllocateBufferType::Temp
                    BufferUsageFlags::TRANSFER_SRC,
                    MemoryPropertyFlags::HOST_COHERENT | MemoryPropertyFlags::HOST_VISIBLE,
                )?;

            data.buffer_manager.copy_data_to_buffer(
                BufferManagerDataType::Data(INDICES),
                BufferManagerCopyType::TempBuffer,
            )?;

            data.buffer_manager
                .allocate_buffer::<IndexBufferSize>(
                    AllocateBufferType::Standard { name: StandardBufferMaps::Indices },
                    BufferUsageFlags::INDEX_BUFFER | BufferUsageFlags::TRANSFER_DST,
                    MemoryPropertyFlags::DEVICE_LOCAL,
                )?;

            data.buffer_manager.copy_data_to_buffer::<IndexBufferSize>(
                BufferManagerDataType::TempBuffer {
                    graphics_queue: data.graphics_queue,
                    command_pool: data.command_pool,
                },
                BufferManagerCopyType::StandardBuffer(StandardBufferMaps::Indices),
            )?;

            data.buffer_manager.free_temp_buffer()
        };

        Ok(())
    }

    pub unsafe fn render(&mut self) -> Result<()> {
        trace!("Rendering");
        let in_flight_fence = self.data.in_flight_fences[self.frame];

        (unsafe {
            self.device
                .wait_for_fences(&[in_flight_fence], true, u64::MAX)
        })?;

        let result = unsafe {
            self.device.acquire_next_image_khr(
                self.data.swapchain,
                u64::MAX,
                self.data.image_available_semaphore[self.frame],
                Fence::null(),
            )
        };

        let image_index = match result {
            Ok((image_index, _)) => image_index as usize,
            Err(ErrorCode::OUT_OF_DATE_KHR) => {
                return unsafe { self.recreate_swapchain() };
            }
            Err(e) => return Err(anyhow!(e)),
        };

        let image_in_flight = self.data.images_in_flight[image_index];
        if !image_in_flight.is_null() {
            (unsafe {
                self.device
                    .wait_for_fences(&[image_in_flight], true, u64::MAX)
            })?;
        }

        self.data.images_in_flight[image_index] = in_flight_fence;

        unsafe { self.update_uniform_buffer(image_index) }?;

        let wait_semaphores = &[self.data.image_available_semaphore[self.frame]];
        let wait_stages = &[PipelineStageFlags::COLOR_ATTACHMENT_OUTPUT];
        let command_buffers = &[self.data.command_buffers[image_index]];
        let signal_semaphores = &[self.data.render_finished_semaphore[image_index]];
        let submit_info = SubmitInfo::builder()
            .wait_semaphores(wait_semaphores)
            .wait_dst_stage_mask(wait_stages)
            .command_buffers(command_buffers)
            .signal_semaphores(signal_semaphores);

        (unsafe { self.device.reset_fences(&[in_flight_fence]) })?;

        (unsafe {
            self.device
                .queue_submit(self.data.graphics_queue, &[submit_info], in_flight_fence)
        })?;

        let swapchains = &[self.data.swapchain];
        let image_indices = &[image_index as u32];
        let present_info = PresentInfoKHR::builder()
            .wait_semaphores(signal_semaphores)
            .swapchains(swapchains)
            .image_indices(image_indices);

        let result = unsafe {
            self.device
                .queue_present_khr(self.data.present_queue, &present_info)
        };
        let changed =
            result == Ok(SuccessCode::SUBOPTIMAL_KHR) || result == Err(ErrorCode::OUT_OF_DATE_KHR);
        if self.resized || changed {
            self.resized = false;
            (unsafe { self.recreate_swapchain() })?;
        } else if let Err(e) = result {
            return Err(anyhow!(e));
        }

        self.frame = (self.frame + 1) % MAX_FRAMES_IN_FLIGHT;

        Ok(())
    }

    unsafe fn update_uniform_buffer(&self, image_index: usize) -> Result<()> {
        let _time = self.start.elapsed().as_secs_f32();

        #[rustfmt::skip]
        let view = Mat4::new(
            1.0, 0.0, 0.0, -self.camera_position[0],
            0.0, 1.0, 0.0, -self.camera_position[1],
            0.0, 0.0, 1.0, -5.0,
            0.0, 0.0, 0.0, 1.0,
        ).transpose();

        #[rustfmt::skip]
        let correction = Mat4::new(
            1.0, 0.0, 0.0, 0.0,
            0.0, -1.0, 0.0, 0.0,
            0.0, 0.0, 0.5, 0.0,
            0.0, 0.0, 0.5, 1.0,
        );

        let proj = correction
            * cgmath::perspective(
                Deg(60.0),
                self.data.swapchain_extent.width as f32 / self.data.swapchain_extent.height as f32,
                0.1,
                10.0,
            );

        // info!("View: {view:?}");
        // info!("Proj: {proj:?}");

        let ubo = UniformBufferObject { view, proj };

        let buffer_pair = self
            .data
            .buffer_manager
            .get_uniform_buffers(UniformBufferMaps::ModelViewProject)[image_index];

        let memory = unsafe {
            self.device.map_memory(
                buffer_pair.memory,
                0,
                size_of::<UniformBufferObject>() as u64,
                MemoryMapFlags::empty(),
            )
        }?;

        unsafe {
            copy_nonoverlapping(&ubo, memory.cast(), 1);
            self.device.unmap_memory(buffer_pair.memory)
        };

        let sbo = SporadicBufferObject { num_instances: 32 };

        let buffer_pair = self
            .data
            .buffer_manager
            .get_uniform_buffers(UniformBufferMaps::SporadicBufferObject)[image_index];

        let memory = unsafe {
            self.device.map_memory(
                buffer_pair.memory,
                0,
                size_of::<SporadicBufferObject>() as u64,
                MemoryMapFlags::empty(),
            )
        }?;

        unsafe {
            copy_nonoverlapping(&sbo, memory.cast(), 1);
            self.device.unmap_memory(buffer_pair.memory)
        }

        Ok(())
    }

    unsafe fn create_descriptor_pool(data: &mut VulkanData) -> Result<()> {
        let sampler_size = DescriptorPoolSize::builder()
            .type_(DescriptorType::COMBINED_IMAGE_SAMPLER)
            .descriptor_count(data.swapchain_images.len() as u32);

        data.descriptor_pool = unsafe {
            data.buffer_manager.create_descriptor_pool(
                data.swapchain_images.len() as u32,
                Some(vec![sampler_size]),
            )?
        };

        Ok(())
    }

    unsafe fn create_descriptor_sets(device: &Device, data: &mut VulkanData) -> Result<()> {
        let layouts = vec![data.descriptor_set_layout; data.swapchain_images.len()];
        let info = DescriptorSetAllocateInfo::builder()
            .descriptor_pool(data.descriptor_pool)
            .set_layouts(&layouts);

        data.descriptor_sets = unsafe { device.allocate_descriptor_sets(&info) }?;

        let ubo_descriptors = unsafe {
            data.buffer_manager
                .create_buffer_descriptor_set::<UniformBufferObject>(
                    0,
                    UniformBufferMaps::ModelViewProject,
                    &data.descriptor_sets,
                )
        };
        let sbo_descriptors = unsafe {
            data.buffer_manager
                .create_buffer_descriptor_set::<SporadicBufferObject>(
                    1,
                    UniformBufferMaps::SporadicBufferObject,
                    &data.descriptor_sets,
                )
        };

        for i in 0..data.swapchain_images.len() {
            let info = DescriptorImageInfo::builder()
                .image_layout(ImageLayout::SHADER_READ_ONLY_OPTIMAL)
                .image_view(data.texture_image_view)
                .sampler(data.texture_sampler);

            let image_info = &[info];
            let sampler_write = WriteDescriptorSet::builder()
                .dst_set(data.descriptor_sets[i])
                .dst_binding(2)
                .dst_array_element(0)
                .descriptor_type(DescriptorType::COMBINED_IMAGE_SAMPLER)
                .image_info(image_info);

            unsafe {
                device.update_descriptor_sets(
                    &[ubo_descriptors[i], sbo_descriptors[i], sampler_write],
                    &[] as &[CopyDescriptorSet],
                )
            };
        }

        Ok(())
    }

    unsafe fn create_depth_objects(
        instance: &Instance,
        device: &Device,
        data: &mut VulkanData,
    ) -> Result<()> {
        let format = unsafe { Self::get_depth_format(instance, data) }?;

        let (depth_image, depth_image_memory) = unsafe {
            Self::create_image(
                instance,
                device,
                data,
                data.swapchain_extent.width,
                data.swapchain_extent.height,
                format,
                ImageTiling::OPTIMAL,
                ImageUsageFlags::DEPTH_STENCIL_ATTACHMENT,
                MemoryPropertyFlags::DEVICE_LOCAL,
            )
        }?;

        data.depth_image = depth_image;
        data.depth_image_memory = depth_image_memory;

        // Image View

        data.depth_image_view = unsafe {
            Self::create_image_view(device, data.depth_image, format, ImageAspectFlags::DEPTH)
        }?;

        (unsafe {
            Self::transition_image_layout(
                device,
                data,
                data.depth_image,
                format,
                ImageLayout::UNDEFINED,
                ImageLayout::DEPTH_STENCIL_ATTACHMENT_OPTIMAL,
            )
        })?;

        Ok(())
    }

    unsafe fn get_supported_format(
        instance: &Instance,
        data: &VulkanData,
        candidates: &[Format],
        tiling: ImageTiling,
        features: FormatFeatureFlags,
    ) -> Result<Format> {
        candidates
            .iter()
            .cloned()
            .find(|f| {
                let properties = unsafe {
                    instance.get_physical_device_format_properties(data.physical_device, *f)
                };

                match tiling {
                    ImageTiling::LINEAR => properties.linear_tiling_features.contains(features),
                    ImageTiling::OPTIMAL => properties.optimal_tiling_features.contains(features),
                    _ => false,
                }
            })
            .ok_or(anyhow!("Failed to find supported format!"))
    }

    unsafe fn get_depth_format(instance: &Instance, data: &VulkanData) -> Result<Format> {
        let candidates = &[
            Format::D32_SFLOAT,
            Format::D32_SFLOAT_S8_UINT,
            Format::D24_UNORM_S8_UINT,
        ];

        unsafe {
            Self::get_supported_format(
                instance,
                data,
                candidates,
                ImageTiling::OPTIMAL,
                FormatFeatureFlags::DEPTH_STENCIL_ATTACHMENT,
            )
        }
    }

    unsafe fn create_texture_image(
        instance: &Instance,
        device: &Device,
        data: &mut VulkanData,
    ) -> Result<()> {
        let image = File::open("resources/tuftie.png")?;

        let decoder = png::Decoder::new(image);
        let mut reader = decoder.read_info()?;

        let mut pixels = vec![0; reader.info().raw_bytes()];
        reader.next_frame(&mut pixels)?;

        let size = reader.info().raw_bytes() as u64;
        let (width, height) = reader.info().size();

        let (staging_buffer, staging_buffer_memory) = unsafe {
            Self::create_buffer(
                instance,
                device,
                data,
                size,
                BufferUsageFlags::TRANSFER_SRC,
                MemoryPropertyFlags::HOST_COHERENT | MemoryPropertyFlags::HOST_VISIBLE,
            )
        }?;

        let memory =
            unsafe { device.map_memory(staging_buffer_memory, 0, size, MemoryMapFlags::empty()) }?;

        unsafe { copy_nonoverlapping(pixels.as_ptr(), memory.cast(), pixels.len()) };

        unsafe { device.unmap_memory(staging_buffer_memory) };

        let (texture_image, texture_image_memory) = unsafe {
            Self::create_image(
                instance,
                device,
                data,
                width,
                height,
                Format::R8G8B8A8_SRGB,
                ImageTiling::OPTIMAL,
                ImageUsageFlags::SAMPLED | ImageUsageFlags::TRANSFER_DST,
                MemoryPropertyFlags::DEVICE_LOCAL,
            )
        }?;

        data.texture_image = texture_image;
        data.texture_image_memory = texture_image_memory;

        (unsafe {
            Self::transition_image_layout(
                device,
                data,
                data.texture_image,
                Format::R8G8B8A8_SRGB,
                ImageLayout::UNDEFINED,
                ImageLayout::TRANSFER_DST_OPTIMAL,
            )
        })?;

        (unsafe {
            Self::copy_buffer_to_image(
                device,
                data,
                staging_buffer,
                data.texture_image,
                width,
                height,
            )
        })?;

        (unsafe {
            Self::transition_image_layout(
                device,
                data,
                data.texture_image,
                Format::R8G8B8A8_SRGB,
                ImageLayout::TRANSFER_DST_OPTIMAL,
                ImageLayout::SHADER_READ_ONLY_OPTIMAL,
            )
        })?;

        // Cleanup

        unsafe { device.destroy_buffer(staging_buffer, None) };
        unsafe { device.free_memory(staging_buffer_memory, None) };

        Ok(())
    }

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

    unsafe fn create_texture_sampler(device: &Device, data: &mut VulkanData) -> Result<()> {
        let info = SamplerCreateInfo::builder()
            .mag_filter(Filter::LINEAR)
            .min_filter(Filter::LINEAR)
            .address_mode_u(SamplerAddressMode::REPEAT)
            .address_mode_v(SamplerAddressMode::REPEAT)
            .address_mode_w(SamplerAddressMode::REPEAT)
            .anisotropy_enable(true)
            .max_anisotropy(16.0)
            .border_color(BorderColor::INT_OPAQUE_BLACK)
            .unnormalized_coordinates(false)
            .compare_enable(false)
            .compare_op(CompareOp::ALWAYS)
            .mipmap_mode(SamplerMipmapMode::LINEAR)
            .mip_lod_bias(0.0)
            .min_lod(0.0)
            .max_lod(0.0);

        data.texture_sampler = unsafe { device.create_sampler(&info, None)? };

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

    pub unsafe fn create_instance(
        window: &Window,
        entry: &Entry,
        data: &mut VulkanData,
    ) -> Result<Instance> {
        let application_info = ApplicationInfo::builder()
            .application_name(b"Factory Game\0")
            .application_version(make_version(1, 0, 0))
            .engine_name(b"Lemgine\0")
            .engine_version(make_version(1, 0, 0))
            .api_version(make_version(1, 0, 0));

        let mut extensions = get_required_instance_extensions(window)
            .iter()
            .map(|e| e.as_ptr())
            .collect::<Vec<_>>();

        if VALIDATION_ENABLED {
            extensions.push(EXT_DEBUG_UTILS_EXTENSION.name.as_ptr());
        }
        extensions.push(KHR_GET_PHYSICAL_DEVICE_PROPERTIES2_EXTENSION.name.as_ptr());

        let flags = if cfg!(target_os = "macos") && entry.version()? >= PORTABILITY_MACOS_VERSION {
            info!("Enabling extensions for macOS portability");
            extensions.push(KHR_PORTABILITY_ENUMERATION_EXTENSION.name.as_ptr());
            InstanceCreateFlags::ENUMERATE_PORTABILITY_KHR
        } else {
            InstanceCreateFlags::empty()
        };

        let available_layers = unsafe {
            entry
                .enumerate_instance_layer_properties()?
                .iter()
                .map(|l| l.layer_name)
                .collect::<HashSet<_>>()
        };

        if VALIDATION_ENABLED && !available_layers.contains(&VALIDATION_LAYER) {
            return Err(anyhow!("Validation layer requested but not supported."));
        }

        let layers = if VALIDATION_ENABLED {
            vec![VALIDATION_LAYER.as_ptr()]
        } else {
            vec![]
        };

        let mut info = InstanceCreateInfo::builder()
            .application_info(&application_info)
            .enabled_extension_names(&extensions)
            .enabled_layer_names(&layers)
            .flags(flags);

        let mut debug_info = DebugUtilsMessengerCreateInfoEXT::builder()
            .message_severity(DebugUtilsMessageSeverityFlagsEXT::all())
            .message_type(
                DebugUtilsMessageTypeFlagsEXT::GENERAL
                    | DebugUtilsMessageTypeFlagsEXT::VALIDATION
                    | DebugUtilsMessageTypeFlagsEXT::PERFORMANCE,
            )
            .user_callback(Some(debug_callback));

        if VALIDATION_ENABLED {
            info = info.push_next(&mut debug_info);
        }

        let instance = unsafe { entry.create_instance(&info, None)? };
        if VALIDATION_ENABLED {
            data.messenger =
                unsafe { instance.create_debug_utils_messenger_ext(&debug_info, None)? };
        }
        Ok(instance)
    }

    pub unsafe fn pick_physical_device(instance: &Instance, data: &mut VulkanData) -> Result<()> {
        for physical_device in unsafe { instance.enumerate_physical_devices() }? {
            let properties = unsafe { instance.get_physical_device_properties(physical_device) };

            if let Err(error) =
                unsafe { Self::check_physical_device(instance, data, physical_device) }
            {
                warn!(
                    "Skipping physical device (`{}`) : {}",
                    properties.device_name, error
                );
            } else {
                info!("Selected physical device (`{}`).", properties.device_name);
                data.physical_device = physical_device;
                return Ok(());
            }
        }

        Err(anyhow!("Failed to find suitable device."))
    }

    /// Get the physical device requirements and check they meet our requirements
    unsafe fn check_physical_device(
        instance: &Instance,
        data: &VulkanData,
        physical_device: PhysicalDevice,
    ) -> Result<()> {
        let properties = unsafe { instance.get_physical_device_properties(physical_device) };

        if properties.device_type != PhysicalDeviceType::DISCRETE_GPU
            && properties.device_type != PhysicalDeviceType::INTEGRATED_GPU
        {
            return Err(anyhow!(SuitabilityError(
                "Only discrete GPUs are supported."
            )));
        }

        let features = unsafe { instance.get_physical_device_features(physical_device) };
        if features.geometry_shader != TRUE {
            return Err(anyhow!(SuitabilityError(
                "Missing geometry shader support."
            )));
        }
        if features.sampler_anisotropy != TRUE {
            return Err(anyhow!(SuitabilityError("No sampler anisotropy.")));
        }

        unsafe { QueueFamilyIndices::get(instance, data, physical_device)? };
        unsafe { Self::check_physical_device_extensions(instance, physical_device) }?;

        let support = unsafe { SwapchainSupport::get(instance, data, physical_device) }?;
        if support.formats.is_empty() || support.present_modes.is_empty() {
            return Err(anyhow!(SuitabilityError("Insufficient swapchain support.")));
        }

        Ok(())
    }

    unsafe fn check_physical_device_extensions(
        instance: &Instance,
        physical_device: PhysicalDevice,
    ) -> Result<()> {
        let extensions =
            unsafe { instance.enumerate_device_extension_properties(physical_device, None) }?
                .iter()
                .map(|e| e.extension_name)
                .collect::<HashSet<_>>();

        if DEVICE_EXTENSIONS.iter().all(|e| extensions.contains(e)) {
            Ok(())
        } else {
            Err(anyhow!(SuitabilityError(
                "Missing required device extensions."
            )))
        }
    }

    unsafe fn create_logical_device(
        entry: &Entry,
        instance: &Instance,
        data: &mut VulkanData,
    ) -> Result<Device> {
        let indices = unsafe { QueueFamilyIndices::get(instance, data, data.physical_device) }?;

        let mut unique_indices = HashSet::new();
        unique_indices.insert(indices.graphics);
        unique_indices.insert(indices.present);

        let queue_priorities = &[1.0];
        let queue_infos = unique_indices
            .iter()
            .map(|i| {
                DeviceQueueCreateInfo::builder()
                    .queue_family_index(*i)
                    .queue_priorities(queue_priorities)
            })
            .collect::<Vec<_>>();

        let layers = if VALIDATION_ENABLED {
            vec![VALIDATION_LAYER.as_ptr()]
        } else {
            vec![]
        };

        let mut extensions = DEVICE_EXTENSIONS
            .iter()
            .map(|n| n.as_ptr())
            .collect::<Vec<_>>();

        if cfg!(target_os = "macos") && entry.version()? >= PORTABILITY_MACOS_VERSION {
            // original tutorial used KHR_PORTABILITY_SUBSET_EXTENSION but I can't find that so maybe this is okay?
            extensions.push(KHR_PORTABILITY_ENUMERATION_EXTENSION.name.as_ptr());
        }

        let features = PhysicalDeviceFeatures::builder().sampler_anisotropy(true);

        let info = DeviceCreateInfo::builder()
            .queue_create_infos(&queue_infos)
            .enabled_layer_names(&layers)
            .enabled_extension_names(&extensions)
            .enabled_features(&features);

        let device = unsafe { instance.create_device(data.physical_device, &info, None) }?;
        data.graphics_queue = unsafe { device.get_device_queue(indices.graphics, 0) };
        data.present_queue = unsafe { device.get_device_queue(indices.present, 0) };

        Ok(device)
    }

    unsafe fn create_swapchain(
        window: &Window,
        instance: &Instance,
        device: &Device,
        data: &mut VulkanData,
    ) -> Result<()> {
        let indices = unsafe { QueueFamilyIndices::get(instance, data, data.physical_device) }?;
        let support = unsafe { SwapchainSupport::get(instance, data, data.physical_device) }?;

        let surface_format = SwapchainSupport::get_swapchain_surface_format(&support.formats);
        let present_mode = SwapchainSupport::get_swapchain_present_mode(&support.present_modes);
        let extent = SwapchainSupport::get_swapchain_extent(window, support.capabilities);

        let mut image_count = support.capabilities.min_image_count + 1;
        data.swapchain_min_image_count = image_count;

        if support.capabilities.max_image_count != 0
            && image_count > support.capabilities.max_image_count
        {
            image_count = support.capabilities.max_image_count;
        }

        let mut queue_family_indices = vec![];
        let image_sharing_mode = if indices.graphics != indices.present {
            queue_family_indices.push(indices.graphics);
            queue_family_indices.push(indices.present);
            SharingMode::CONCURRENT
        } else {
            SharingMode::EXCLUSIVE
        };

        let info = SwapchainCreateInfoKHR::builder()
            .surface(data.surface)
            .min_image_count(image_count)
            .image_format(surface_format.format)
            .image_color_space(surface_format.color_space)
            .image_extent(extent)
            .image_array_layers(1)
            .image_usage(ImageUsageFlags::COLOR_ATTACHMENT)
            .image_sharing_mode(image_sharing_mode)
            .queue_family_indices(&queue_family_indices)
            .pre_transform(support.capabilities.current_transform)
            .composite_alpha(CompositeAlphaFlagsKHR::OPAQUE)
            .present_mode(present_mode)
            .clipped(true)
            .old_swapchain(SwapchainKHR::null());

        data.swapchain = unsafe { device.create_swapchain_khr(&info, None) }?;
        data.swapchain_images = unsafe { device.get_swapchain_images_khr(data.swapchain) }?;
        data.swapchain_format = surface_format.format;
        data.swapchain_extent = extent;

        Ok(())
    }

    unsafe fn create_descriptor_set_layout(data: &mut VulkanData) -> Result<()> {
        data.buffer_manager
            .setup_uniform_buffer(UniformBufferMaps::ModelViewProject);
        data.buffer_manager
            .setup_uniform_buffer(UniformBufferMaps::SporadicBufferObject);

        data.descriptor_set_layout = unsafe {
            data.buffer_manager.create_descriptor_set_layout(Some(vec![
                DescriptorSetLayoutBinding::builder()
                    .binding(2)
                    .descriptor_type(DescriptorType::COMBINED_IMAGE_SAMPLER)
                    .descriptor_count(1)
                    .stage_flags(ShaderStageFlags::FRAGMENT),
            ]))?
        };

        Ok(())
    }

    unsafe fn create_pipeline(device: &Device, data: &mut VulkanData) -> Result<()> {
        let vert = include_bytes!("../../shaders/vert.spv");
        let frag = include_bytes!("../../shaders/frag.spv");

        let vert_shader_module = unsafe { Self::create_shader_module(device, vert)? };
        let frag_shader_module = unsafe { Self::create_shader_module(device, frag)? };

        let vert_stage = PipelineShaderStageCreateInfo::builder()
            .stage(ShaderStageFlags::VERTEX)
            .module(vert_shader_module)
            .name(b"main\0");

        let frag_stage = PipelineShaderStageCreateInfo::builder()
            .stage(ShaderStageFlags::FRAGMENT)
            .module(frag_shader_module)
            .name(b"main\0");

        let binding_descriptions = &[Vertex::binding_description()];
        let attribute_descriptions = Vertex::attribute_descriptions();
        let vertex_input_state = PipelineVertexInputStateCreateInfo::builder()
            .vertex_binding_descriptions(binding_descriptions)
            .vertex_attribute_descriptions(&attribute_descriptions);

        let input_assembly_state = PipelineInputAssemblyStateCreateInfo::builder()
            .topology(PrimitiveTopology::TRIANGLE_LIST)
            .primitive_restart_enable(false);

        let viewport = Viewport::builder()
            .x(0.0)
            .y(0.0)
            .width(data.swapchain_extent.width as f32)
            .height(data.swapchain_extent.height as f32)
            .min_depth(0.0)
            .max_depth(1.0);

        let scissor = Rect2D::builder()
            .offset(Offset2D { x: 0, y: 0 })
            .extent(data.swapchain_extent);

        let viewports = &[viewport];
        let scissors = &[scissor];
        let viewport_state = PipelineViewportStateCreateInfo::builder()
            .viewports(viewports)
            .scissors(scissors);

        let rasterization_state = PipelineRasterizationStateCreateInfo::builder()
            .depth_clamp_enable(false)
            .rasterizer_discard_enable(false)
            .polygon_mode(PolygonMode::FILL)
            .line_width(1.0)
            .cull_mode(CullModeFlags::BACK)
            .front_face(FrontFace::COUNTER_CLOCKWISE)
            .depth_bias_enable(false);

        let multisample_state = PipelineMultisampleStateCreateInfo::builder()
            .sample_shading_enable(false)
            .rasterization_samples(SampleCountFlags::_1);

        let attachment = PipelineColorBlendAttachmentState::builder()
            .color_write_mask(ColorComponentFlags::all())
            .blend_enable(false)
            .src_color_blend_factor(BlendFactor::ONE) // Optional
            .dst_color_blend_factor(BlendFactor::ZERO) // Optional
            .color_blend_op(BlendOp::ADD) // Optional
            .src_alpha_blend_factor(BlendFactor::ONE) // Optional
            .dst_alpha_blend_factor(BlendFactor::ZERO) // Optional
            .alpha_blend_op(BlendOp::ADD); // Optional

        let attachments = &[attachment];
        let color_blend_state = PipelineColorBlendStateCreateInfo::builder()
            .logic_op_enable(false)
            .logic_op(LogicOp::COPY)
            .attachments(attachments)
            .blend_constants([0.0, 0.0, 0.0, 0.0]);

        let vert_push_constant_range = PushConstantRange::builder()
            .stage_flags(ShaderStageFlags::VERTEX)
            .offset(0)
            .size(64);

        let set_layouts = &[data.descriptor_set_layout];
        let push_constant_ranges = &[vert_push_constant_range];
        let layout_info = PipelineLayoutCreateInfo::builder()
            .set_layouts(set_layouts)
            .push_constant_ranges(push_constant_ranges);

        data.pipeline_layout = unsafe { device.create_pipeline_layout(&layout_info, None) }?;

        let depth_stencil_state = PipelineDepthStencilStateCreateInfo::builder()
            .depth_test_enable(true)
            .depth_write_enable(true)
            .depth_compare_op(CompareOp::LESS)
            .depth_bounds_test_enable(false)
            .min_depth_bounds(0.0) // Optional.
            .max_depth_bounds(1.0) // Optional.
            .stencil_test_enable(false);

        let stages = &[vert_stage, frag_stage];
        let info = GraphicsPipelineCreateInfo::builder()
            .stages(stages)
            .vertex_input_state(&vertex_input_state)
            .input_assembly_state(&input_assembly_state)
            .viewport_state(&viewport_state)
            .rasterization_state(&rasterization_state)
            .multisample_state(&multisample_state)
            .depth_stencil_state(&depth_stencil_state)
            .color_blend_state(&color_blend_state)
            .layout(data.pipeline_layout)
            .render_pass(data.render_pass)
            .subpass(0);

        data.pipeline =
            unsafe { device.create_graphics_pipelines(PipelineCache::null(), &[info], None) }?.0[0];

        unsafe { device.destroy_shader_module(vert_shader_module, None) };
        unsafe { device.destroy_shader_module(frag_shader_module, None) };

        Ok(())
    }

    unsafe fn create_shader_module(device: &Device, bytecode: &[u8]) -> Result<ShaderModule> {
        let bytecode = Bytecode::new(bytecode).unwrap();

        let info = ShaderModuleCreateInfo::builder()
            .code(bytecode.code())
            .code_size(bytecode.code_size());

        Ok(unsafe { device.create_shader_module(&info, None) }?)
    }

    unsafe fn create_render_pass(
        instance: &Instance,
        device: &Device,
        data: &mut VulkanData,
    ) -> Result<()> {
        let color_attachment = AttachmentDescription::builder()
            .format(data.swapchain_format)
            .samples(SampleCountFlags::_1)
            .load_op(AttachmentLoadOp::CLEAR)
            .store_op(AttachmentStoreOp::STORE)
            .stencil_load_op(AttachmentLoadOp::DONT_CARE)
            .stencil_store_op(AttachmentStoreOp::DONT_CARE)
            .initial_layout(ImageLayout::UNDEFINED)
            .final_layout(ImageLayout::PRESENT_SRC_KHR);

        let color_attachment_ref = AttachmentReference::builder()
            .attachment(0)
            .layout(ImageLayout::COLOR_ATTACHMENT_OPTIMAL);

        let color_attachments = &[color_attachment_ref];

        let depth_stencil_attachment = AttachmentDescription::builder()
            .format(unsafe { Self::get_depth_format(instance, data) }?)
            .samples(SampleCountFlags::_1)
            .load_op(AttachmentLoadOp::CLEAR)
            .store_op(AttachmentStoreOp::DONT_CARE)
            .stencil_load_op(AttachmentLoadOp::DONT_CARE)
            .stencil_store_op(AttachmentStoreOp::DONT_CARE)
            .initial_layout(ImageLayout::UNDEFINED)
            .final_layout(ImageLayout::DEPTH_STENCIL_ATTACHMENT_OPTIMAL);
        let depth_stencil_attachment_ref = AttachmentReference::builder()
            .attachment(1)
            .layout(ImageLayout::DEPTH_STENCIL_ATTACHMENT_OPTIMAL);

        let subpass = SubpassDescription::builder()
            .pipeline_bind_point(PipelineBindPoint::GRAPHICS)
            .color_attachments(color_attachments)
            .depth_stencil_attachment(&depth_stencil_attachment_ref);

        let dependency = SubpassDependency::builder()
            .src_subpass(SUBPASS_EXTERNAL)
            .dst_subpass(0)
            .src_stage_mask(
                PipelineStageFlags::COLOR_ATTACHMENT_OUTPUT
                    | PipelineStageFlags::EARLY_FRAGMENT_TESTS,
            )
            .src_access_mask(AccessFlags::empty())
            .dst_stage_mask(
                PipelineStageFlags::COLOR_ATTACHMENT_OUTPUT
                    | PipelineStageFlags::EARLY_FRAGMENT_TESTS,
            )
            .dst_access_mask(
                AccessFlags::COLOR_ATTACHMENT_WRITE | AccessFlags::DEPTH_STENCIL_ATTACHMENT_WRITE,
            );

        let attachments = &[color_attachment, depth_stencil_attachment];
        let subpasses = &[subpass];
        let dependencies = &[dependency];
        let info = RenderPassCreateInfo::builder()
            .attachments(attachments)
            .subpasses(subpasses)
            .dependencies(dependencies);

        data.render_pass = unsafe { device.create_render_pass(&info, None) }?;

        Ok(())
    }

    unsafe fn create_framebuffers(device: &Device, data: &mut VulkanData) -> Result<()> {
        data.framebuffers = data
            .swapchain_image_views
            .iter()
            .map(|i| {
                let attachments = &[*i, data.depth_image_view];
                let create_info = FramebufferCreateInfo::builder()
                    .render_pass(data.render_pass)
                    .attachments(attachments)
                    .width(data.swapchain_extent.width)
                    .height(data.swapchain_extent.height)
                    .layers(1);

                unsafe { device.create_framebuffer(&create_info, None) }
            })
            .collect::<Result<Vec<_>, _>>()?;

        Ok(())
    }

    unsafe fn create_command_pool(
        instance: &Instance,
        device: &Device,
        data: &mut VulkanData,
    ) -> Result<()> {
        let indices = unsafe { QueueFamilyIndices::get(instance, data, data.physical_device) }?;

        let info = CommandPoolCreateInfo::builder()
            .flags(CommandPoolCreateFlags::empty()) // Optional.
            .queue_family_index(indices.graphics);

        data.command_pool = unsafe { device.create_command_pool(&info, None) }?;

        Ok(())
    }

    unsafe fn create_command_buffers(
        device: &Device,
        data: &mut VulkanData,
        gui: &mut GuiApp,
    ) -> Result<()> {
        let allocate_info = CommandBufferAllocateInfo::builder()
            .command_pool(data.command_pool)
            .level(CommandBufferLevel::PRIMARY)
            .command_buffer_count(data.framebuffers.len() as u32);

        data.command_buffers = unsafe { device.allocate_command_buffers(&allocate_info) }?;

        #[rustfmt::skip]
        let model = Mat4::new(
            1.0, 0.0, 0.0, -0.5,
            0.0, 1.0, 0.0, 0.5,
            0.0, 0.0, 1.0, 0.0,
            0.0, 0.0, 0.0, 1.0
        ).transpose();
        // let model = Mat4::identity();

        let model_bytes: &[u8] = unsafe {
            std::slice::from_raw_parts(&model as *const Mat4 as *const u8, size_of::<Mat4>())
        };

        for (i, command_buffer) in data.command_buffers.iter().enumerate() {
            let inheritance = CommandBufferInheritanceInfo::builder();

            let info = CommandBufferBeginInfo::builder()
                .flags(CommandBufferUsageFlags::empty()) // Optional.
                .inheritance_info(&inheritance); // Optional.

            (unsafe { device.begin_command_buffer(*command_buffer, &info) })?;

            let render_area = Rect2D::builder()
                .offset(Offset2D::default())
                .extent(data.swapchain_extent);

            let color_clear_value = ClearValue {
                color: ClearColorValue {
                    float32: [0.0, 0.0, 0.0, 1.0],
                },
            };

            let depth_clear_value = ClearValue {
                depth_stencil: ClearDepthStencilValue {
                    depth: 1.0,
                    stencil: 0,
                },
            };

            let clear_values = &[color_clear_value, depth_clear_value];
            let info = RenderPassBeginInfo::builder()
                .render_pass(data.render_pass)
                .framebuffer(data.framebuffers[i])
                .render_area(render_area)
                .clear_values(clear_values);

            unsafe {
                device.cmd_begin_render_pass(*command_buffer, &info, SubpassContents::INLINE);
                device.cmd_bind_pipeline(
                    *command_buffer,
                    PipelineBindPoint::GRAPHICS,
                    data.pipeline,
                );
                device.cmd_bind_vertex_buffers(
                    *command_buffer,
                    0,
                    &[data
                        .buffer_manager
                        .get_standard_buffer(StandardBufferMaps::Vertices)
                        .buffer],
                    &[0],
                );
                device.cmd_bind_index_buffer(
                    *command_buffer,
                    data.buffer_manager
                        .get_standard_buffer(StandardBufferMaps::Indices)
                        .buffer,
                    0,
                    IndexType::UINT16,
                );
                device.cmd_bind_descriptor_sets(
                    *command_buffer,
                    PipelineBindPoint::GRAPHICS,
                    data.pipeline_layout,
                    0,
                    &[data.descriptor_sets[i]],
                    &[],
                );
                device.cmd_push_constants(
                    *command_buffer,
                    data.pipeline_layout,
                    ShaderStageFlags::VERTEX,
                    0,
                    model_bytes,
                );
                device.cmd_draw_indexed(*command_buffer, INDICES.len() as u32, 1, 0, 0, 0);

                device.cmd_end_render_pass(*command_buffer);
                device.end_command_buffer(*command_buffer)?;
            };
        }

        Ok(())
    }

    unsafe fn create_sync_objects(device: &Device, data: &mut VulkanData) -> Result<()> {
        let semaphore_info = SemaphoreCreateInfo::builder();
        let fence_info = FenceCreateInfo::builder().flags(FenceCreateFlags::SIGNALED);

        for _ in 0..data.swapchain_min_image_count {
            data.image_available_semaphore
                .push(unsafe { device.create_semaphore(&semaphore_info, None) }?);
            data.render_finished_semaphore
                .push(unsafe { device.create_semaphore(&semaphore_info, None) }?);

            data.in_flight_fences
                .push(unsafe { device.create_fence(&fence_info, None) }?);
        }

        data.images_in_flight = data
            .swapchain_images
            .iter()
            .map(|_| Fence::null())
            .collect();

        Ok(())
    }

    unsafe fn recreate_swapchain(&mut self) -> Result<()> {
        debug!("Recreating Swapchain");
        unsafe {
            self.device.device_wait_idle()?;
            self.destroy_swapchain();
            Self::create_swapchain(&self.window, &self.instance, &self.device, &mut self.data)?;
            Self::create_swapchain_image_views(&self.device, &mut self.data)?;
            Self::create_render_pass(&self.instance, &self.device, &mut self.data)?;
            Self::create_pipeline(&self.device, &mut self.data)?;
            Self::create_depth_objects(&self.instance, &self.device, &mut self.data)?;
            Self::create_framebuffers(&self.device, &mut self.data)?;
            Self::create_uniform_buffers(&mut self.data)?;
            Self::create_descriptor_pool(&mut self.data)?;
            Self::create_descriptor_sets(&self.device, &mut self.data)?;
            Self::create_command_buffers(&self.device, &mut self.data)?;
        }
        self.data
            .images_in_flight
            .resize(self.data.swapchain_images.len(), Fence::null());
        Ok(())
    }

    unsafe fn create_uniform_buffers(data: &mut VulkanData) -> Result<()> {
        unsafe {
            data.buffer_manager
                .free_uniform_buffers(UniformBufferMaps::ModelViewProject);
            data.buffer_manager
                .free_uniform_buffers(UniformBufferMaps::SporadicBufferObject);
        }

        for _ in 0..data.swapchain_images.len() {
            unsafe {
                data.buffer_manager
                    .allocate_buffer::<UniformBufferObject>(
                        AllocateBufferType::Uniform { name: UniformBufferMaps::ModelViewProject },
                        BufferUsageFlags::UNIFORM_BUFFER,
                        MemoryPropertyFlags::HOST_COHERENT | MemoryPropertyFlags::HOST_VISIBLE,
                    )?;
                data.buffer_manager
                    .allocate_buffer::<SporadicBufferObject>(
                        AllocateBufferType::Uniform { name: UniformBufferMaps::SporadicBufferObject },
                        BufferUsageFlags::UNIFORM_BUFFER,
                        MemoryPropertyFlags::HOST_COHERENT | MemoryPropertyFlags::HOST_VISIBLE,
                    )?;
            };
        }

        Ok(())
    }

    unsafe fn create_buffer(
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

    unsafe fn create_vertex_buffer(data: &mut VulkanData) -> Result<()> {
        unsafe {
            type VertexBufferSize = [Vertex; VERTICES.len()];

            data.buffer_manager
                .allocate_buffer::<VertexBufferSize>(
                    AllocateBufferType::Temp,
                    BufferUsageFlags::TRANSFER_SRC,
                    MemoryPropertyFlags::HOST_COHERENT | MemoryPropertyFlags::HOST_VISIBLE,
                )?;

            data.buffer_manager.copy_data_to_buffer(
                BufferManagerDataType::Data(&VERTICES),
                BufferManagerCopyType::TempBuffer,
            )?;

            data.buffer_manager
                .allocate_buffer::<VertexBufferSize>(
                    AllocateBufferType::Standard { name: StandardBufferMaps::Vertices},
                    BufferUsageFlags::VERTEX_BUFFER | BufferUsageFlags::TRANSFER_DST,
                    MemoryPropertyFlags::DEVICE_LOCAL,
                )?;

            data.buffer_manager
                .copy_data_to_buffer::<VertexBufferSize>(
                    BufferManagerDataType::TempBuffer {
                        graphics_queue: data.graphics_queue,
                        command_pool: data.command_pool,
                    },
                    BufferManagerCopyType::StandardBuffer(StandardBufferMaps::Vertices),
                )?;

            data.buffer_manager.free_temp_buffer()
        };

        Ok(())
    }

    unsafe fn create_index_buffer(data: &mut VulkanData) -> Result<()> {
        unsafe {
            type IndexBufferSize = [u16; INDICES.len()];

            data.buffer_manager
                .allocate_buffer::<IndexBufferSize>(
                    AllocateBufferType::Temp,
                    BufferUsageFlags::TRANSFER_SRC,
                    MemoryPropertyFlags::HOST_COHERENT | MemoryPropertyFlags::HOST_VISIBLE,
                )?;

            data.buffer_manager.copy_data_to_buffer(
                BufferManagerDataType::Data(INDICES),
                BufferManagerCopyType::TempBuffer,
            )?;

            data.buffer_manager
                .allocate_buffer::<IndexBufferSize>(
                    AllocateBufferType::Standard { name: StandardBufferMaps::Indices },
                    BufferUsageFlags::INDEX_BUFFER | BufferUsageFlags::TRANSFER_DST,
                    MemoryPropertyFlags::DEVICE_LOCAL,
                )?;

            data.buffer_manager.copy_data_to_buffer::<IndexBufferSize>(
                BufferManagerDataType::TempBuffer {
                    graphics_queue: data.graphics_queue,
                    command_pool: data.command_pool,
                },
                BufferManagerCopyType::StandardBuffer(StandardBufferMaps::Indices),
            )?;

            data.buffer_manager.free_temp_buffer()
        };

        Ok(())
    }

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

    unsafe fn create_texture_image_view(device: &Device, data: &mut VulkanData) -> Result<()> {
        data.texture_image_view = unsafe {
            Self::create_image_view(
                device,
                data.texture_image,
                Format::R8G8B8A8_SRGB,
                ImageAspectFlags::COLOR,
            )
        }?;

        Ok(())
    }

    unsafe fn create_swapchain_image_views(device: &Device, data: &mut VulkanData) -> Result<()> {
        data.swapchain_image_views = data
            .swapchain_images
            .iter()
            .map(|i| unsafe {
                Self::create_image_view(device, *i, data.swapchain_format, ImageAspectFlags::COLOR)
            })
            .collect::<Result<Vec<_>, _>>()?;

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

    unsafe fn destroy_swapchain(&mut self) {
        debug!("Destroying Swapchain");
        unsafe {
            self.device
                .destroy_image_view(self.data.depth_image_view, None);
            self.device.free_memory(self.data.depth_image_memory, None);
            self.device.destroy_image(self.data.depth_image, None);
            self.device
                .destroy_descriptor_pool(self.data.descriptor_pool, None);
            self.device
                .free_command_buffers(self.data.command_pool, &self.data.command_buffers);
            self.data
                .framebuffers
                .iter()
                .for_each(|f| self.device.destroy_framebuffer(*f, None));
            self.device.destroy_pipeline(self.data.pipeline, None);
            self.device
                .destroy_pipeline_layout(self.data.pipeline_layout, None);
            self.device.destroy_render_pass(self.data.render_pass, None);
            self.data
                .swapchain_image_views
                .iter()
                .for_each(|v| self.device.destroy_image_view(*v, None));
            self.device.destroy_swapchain_khr(self.data.swapchain, None)
        };
    }

    pub unsafe fn destroy(&mut self) {
        info!("Destroying VulkanApp");
        unsafe {
            self.data
                .buffer_manager
                .free_uniform_buffers(UniformBufferMaps::ModelViewProject);
            self.data
                .buffer_manager
                .free_uniform_buffers(UniformBufferMaps::SporadicBufferObject);
            self.device.device_wait_idle().unwrap();

            self.destroy_swapchain();
            self.device.destroy_sampler(self.data.texture_sampler, None);
            self.device
                .destroy_image_view(self.data.texture_image_view, None);
            self.device.destroy_image(self.data.texture_image, None);
            self.device
                .free_memory(self.data.texture_image_memory, None);
            self.device
                .destroy_descriptor_set_layout(self.data.descriptor_set_layout, None);
            self.data
                .buffer_manager
                .free_standard_buffer(StandardBufferMaps::Vertices);
            self.data
                .buffer_manager
                .free_standard_buffer(StandardBufferMaps::Indices);

            self.data
                .in_flight_fences
                .iter()
                .for_each(|f| self.device.destroy_fence(*f, None));
            self.data
                .render_finished_semaphore
                .iter()
                .for_each(|s| self.device.destroy_semaphore(*s, None));
            self.data
                .image_available_semaphore
                .iter()
                .for_each(|s| self.device.destroy_semaphore(*s, None));
            self.device
                .destroy_command_pool(self.data.command_pool, None);
            self.device.destroy_device(None);
            self.instance.destroy_surface_khr(self.data.surface, None);

            if VALIDATION_ENABLED {
                self.instance
                    .destroy_debug_utils_messenger_ext(self.data.messenger, None);
            }

            self.instance.destroy_instance(None);
        }
    }
}

#[derive(Copy, Clone, Debug)]
struct QueueFamilyIndices {
    graphics: u32,
    present: u32,
}

impl QueueFamilyIndices {
    unsafe fn get(
        instance: &Instance,
        data: &VulkanData,
        physical_device: PhysicalDevice,
    ) -> Result<Self> {
        let properties =
            unsafe { instance.get_physical_device_queue_family_properties(physical_device) };

        let graphics = properties
            .iter()
            .position(|p| p.queue_flags.contains(QueueFlags::GRAPHICS))
            .map(|i| i as u32);

        let mut present = None;
        for (index, _properties) in properties.iter().enumerate() {
            if unsafe {
                instance.get_physical_device_surface_support_khr(
                    physical_device,
                    index as u32,
                    data.surface,
                )?
            } {
                present = Some(index as u32);
                break;
            }
        }

        if let (Some(graphics), Some(present)) = (graphics, present) {
            Ok(Self { graphics, present })
        } else {
            Err(anyhow!(SuitabilityError(
                "Missing required queue families."
            )))
        }
    }
}

#[derive(Clone, Debug)]
struct SwapchainSupport {
    capabilities: SurfaceCapabilitiesKHR,
    formats: Vec<SurfaceFormatKHR>,
    present_modes: Vec<PresentModeKHR>,
}

impl SwapchainSupport {
    unsafe fn get(
        instance: &Instance,
        data: &VulkanData,
        physical_device: PhysicalDevice,
    ) -> Result<Self> {
        Ok(Self {
            capabilities: unsafe {
                instance.get_physical_device_surface_capabilities_khr(physical_device, data.surface)
            }?,
            formats: unsafe {
                instance.get_physical_device_surface_formats_khr(physical_device, data.surface)
            }?,
            present_modes: unsafe {
                instance
                    .get_physical_device_surface_present_modes_khr(physical_device, data.surface)?
            },
        })
    }
    fn get_swapchain_surface_format(formats: &[SurfaceFormatKHR]) -> SurfaceFormatKHR {
        formats
            .iter()
            .cloned()
            .find(|f| {
                f.format == Format::B8G8R8_SRGB && f.color_space == ColorSpaceKHR::SRGB_NONLINEAR
            })
            .unwrap_or_else(|| formats[0])
    }
    fn get_swapchain_present_mode(present_modes: &[PresentModeKHR]) -> PresentModeKHR {
        present_modes
            .iter()
            .cloned()
            .find(|m| *m == PresentModeKHR::MAILBOX)
            .unwrap_or(PresentModeKHR::FIFO)
    }
    fn get_swapchain_extent(window: &Window, capabilities: SurfaceCapabilitiesKHR) -> Extent2D {
        if capabilities.current_extent.width != u32::MAX {
            capabilities.current_extent
        } else {
            Extent2D::builder()
                .width(window.inner_size().width.clamp(
                    capabilities.min_image_extent.width,
                    capabilities.max_image_extent.width,
                ))
                .height(window.inner_size().height.clamp(
                    capabilities.min_image_extent.height,
                    capabilities.max_image_extent.height,
                ))
                .build()
        }
    }
}
