use anyhow::{Result, anyhow};
use cgmath::{Deg, point3, vec3};
use image::imageops::FilterType::Lanczos3;
use log::*;
use std::{
    collections::HashSet,
    ffi::{CStr, c_void},
    intrinsics::copy_nonoverlapping,
    time::Instant,
};
use thiserror::Error;
use vulkanalia::{
    Device, Entry, Instance, Version,
    bytecode::Bytecode,
    loader::{LIBRARY, LibloadingLoader},
    vk::{
        AccessFlags, ApplicationInfo, AttachmentDescription, AttachmentLoadOp, AttachmentReference,
        AttachmentStoreOp, BlendFactor, BlendOp, Bool32, Buffer, BufferCopy, BufferCreateFlags,
        BufferCreateInfo, BufferUsageFlags, ClearColorValue, ClearValue, ColorComponentFlags,
        ColorSpaceKHR, CommandBuffer, CommandBufferAllocateInfo, CommandBufferBeginInfo,
        CommandBufferInheritanceInfo, CommandBufferLevel, CommandBufferUsageFlags, CommandPool,
        CommandPoolCreateFlags, CommandPoolCreateInfo, ComponentMapping, ComponentSwizzle,
        CompositeAlphaFlagsKHR, CopyDescriptorSet, CullModeFlags,
        DebugUtilsMessageSeverityFlagsEXT, DebugUtilsMessageTypeFlagsEXT,
        DebugUtilsMessengerCallbackDataEXT, DebugUtilsMessengerCreateInfoEXT,
        DebugUtilsMessengerEXT, DescriptorBufferInfo, DescriptorPool, DescriptorPoolCreateInfo,
        DescriptorPoolSize, DescriptorSet, DescriptorSetAllocateInfo, DescriptorSetLayout,
        DescriptorSetLayoutBinding, DescriptorSetLayoutCreateInfo, DescriptorType,
        DeviceCreateInfo, DeviceMemory, DeviceQueueCreateInfo, DeviceSize, DeviceV1_0,
        EXT_DEBUG_UTILS_EXTENSION, EntryV1_0, ErrorCode, ExtDebugUtilsExtensionInstanceCommands,
        ExtensionName, Extent2D, FALSE, Fence, FenceCreateFlags, FenceCreateInfo, Format,
        Framebuffer, FramebufferCreateInfo, FrontFace, GraphicsPipelineCreateInfo, Handle,
        HasBuilder, Image, ImageAspectFlags, ImageLayout, ImageSubresourceRange, ImageUsageFlags,
        ImageView, ImageViewCreateInfo, ImageViewType, IndexType, InstanceCreateFlags,
        InstanceCreateInfo, InstanceV1_0, KHR_GET_PHYSICAL_DEVICE_PROPERTIES2_EXTENSION,
        KHR_PORTABILITY_ENUMERATION_EXTENSION, KHR_SWAPCHAIN_EXTENSION,
        KhrSurfaceExtensionInstanceCommands, KhrSwapchainExtensionDeviceCommands, LogicOp,
        MemoryAllocateInfo, MemoryMapFlags, MemoryPropertyFlags, MemoryRequirements, Offset2D,
        PhysicalDevice, PhysicalDeviceFeatures, PhysicalDeviceType, Pipeline, PipelineBindPoint,
        PipelineCache, PipelineColorBlendAttachmentState, PipelineColorBlendStateCreateInfo,
        PipelineInputAssemblyStateCreateInfo, PipelineLayout, PipelineLayoutCreateInfo,
        PipelineMultisampleStateCreateInfo, PipelineRasterizationStateCreateInfo,
        PipelineShaderStageCreateInfo, PipelineStageFlags, PipelineVertexInputStateCreateInfo,
        PipelineViewportStateCreateInfo, PolygonMode, PresentInfoKHR, PresentModeKHR,
        PrimitiveTopology, Queue, QueueFlags, Rect2D, RenderPass, RenderPassBeginInfo,
        RenderPassCreateInfo, SUBPASS_EXTERNAL, SampleCountFlags, Semaphore, SemaphoreCreateInfo,
        ShaderModule, ShaderModuleCreateInfo, ShaderStageFlags, SharingMode, SubmitInfo,
        SubpassContents, SubpassDependency, SubpassDescription, SuccessCode,
        SurfaceCapabilitiesKHR, SurfaceFormatKHR, SurfaceKHR, SwapchainCreateInfoKHR, SwapchainKHR,
        TRUE, Viewport, WriteDescriptorSet, make_version,
    },
    window::{create_surface, get_required_instance_extensions},
};
use winit::window::Window;

use crate::engine::vertex::{INDICES, Mat4, UniformBufferObject, VERTICES, Vertex};

const PORTABILITY_MACOS_VERSION: Version = Version::new(1, 3, 216);

const VALIDATION_ENABLED: bool = cfg!(debug_assertions);
const VALIDATION_LAYER: ExtensionName = ExtensionName::from_bytes(b"VK_LAYER_KHRONOS_validation");

const DEVICE_EXTENSIONS: &[ExtensionName] = &[KHR_SWAPCHAIN_EXTENSION.name];

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

#[derive(Clone, Debug)]
pub struct VulkanApp {
    pub entry: Entry,
    pub instance: Instance,
    pub device: Device,
    pub data: VulkanData,
    pub frame: usize,
    pub resized: bool,
    start: Instant,
}

#[derive(Clone, Debug, Default)]
pub struct VulkanData {
    messenger: DebugUtilsMessengerEXT,
    physical_device: PhysicalDevice,
    graphics_queue: Queue,
    present_queue: Queue,
    surface: SurfaceKHR,
    swapchain: SwapchainKHR,
    swapchain_format: Format,
    swapchain_extent: Extent2D,
    swapchain_images: Vec<Image>,
    swapchain_image_views: Vec<ImageView>,
    descriptor_set_layout: DescriptorSetLayout,
    pipeline_layout: PipelineLayout,
    render_pass: RenderPass,
    pipeline: Pipeline,
    framebuffers: Vec<Framebuffer>,
    command_pool: CommandPool,
    command_buffers: Vec<CommandBuffer>,
    image_available_semaphore: Vec<Semaphore>,
    render_finished_semaphore: Vec<Semaphore>,
    in_flight_fences: Vec<Fence>,
    images_in_flight: Vec<Fence>,
    vertex_buffer: Buffer,
    vertex_buffer_memory: DeviceMemory,
    index_buffer: Buffer,
    index_buffer_memory: DeviceMemory,
    uniform_buffers: Vec<Buffer>,
    uniform_buffers_memory: Vec<DeviceMemory>,
    descriptor_pool: DescriptorPool,
    descriptor_sets: Vec<DescriptorSet>,
    swapchain_min_image_count: u32,
}

impl VulkanApp {
    pub unsafe fn create(window: &Window) -> Result<Self> {
        unsafe {
            let loader = LibloadingLoader::new(LIBRARY)?;
            let entry = Entry::new(loader).map_err(|b| anyhow!("{}", b))?;
            let mut data = VulkanData::default();
            let instance = Self::create_instance(window, &entry, &mut data)?;
            data.surface = create_surface(&instance, &window, &window)?;
            Self::pick_physical_device(&instance, &mut data)?;
            let device = Self::create_logical_device(&entry, &instance, &mut data)?;
            Self::create_swapchain(window, &instance, &device, &mut data)?;
            Self::create_swapchain_image_views(&device, &mut data)?;
            Self::create_render_pass(&instance, &device, &mut data)?;
            Self::create_descriptor_set_layout(&device, &mut data)?;
            Self::create_pipeline(&device, &mut data)?;
            Self::create_framebuffers(&device, &mut data)?;
            Self::create_command_pool(&instance, &device, &mut data)?;
            Self::create_vertex_buffer(&instance, &device, &mut data)?;
            Self::create_index_buffer(&instance, &device, &mut data)?;
            Self::create_uniform_buffers(&instance, &device, &mut data)?;
            Self::create_descriptor_pool(&device, &mut data)?;
            Self::create_descriptor_sets(&device, &mut data)?;
            Self::create_command_buffers(&device, &mut data)?;
            Self::create_sync_objects(&device, &mut data)?;
            info!("Woo created everything, hard work ain't it?");
            Ok(Self {
                entry,
                instance,
                device,
                data,
                frame: 0,
                resized: false,
                start: Instant::now(),
            })
        }
    }

    pub unsafe fn render(&mut self, window: &Window) -> Result<()> {
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
            Err(ErrorCode::OUT_OF_DATE_KHR) => return unsafe { self.recreate_swapchain(window) },
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
            (unsafe { self.recreate_swapchain(window) })?;
        } else if let Err(e) = result {
            return Err(anyhow!(e));
        }

        self.frame = (self.frame + 1) % MAX_FRAMES_IN_FLIGHT;

        Ok(())
    }

    unsafe fn update_uniform_buffer(&self, image_index: usize) -> Result<()> {
        let time = self.start.elapsed().as_secs_f32();

        let model = Mat4::from_axis_angle(vec3(0.0, 0.0, 1.0), Deg(90.0) * time);

        let view = Mat4::look_at_rh(
            point3(2.0, 2.0, 2.0),
            point3(0.0, 0.0, 0.0),
            vec3(0.0, 0.0, 1.0),
        );

        let mut proj = cgmath::perspective(
            Deg(45.0),
            self.data.swapchain_extent.width as f32 / self.data.swapchain_extent.height as f32,
            0.1,
            10.0,
        );

        proj[1][1] *= -1.0;

        let ubo = UniformBufferObject { model, view, proj };

        let memory = unsafe {
            self.device.map_memory(
                self.data.uniform_buffers_memory[image_index],
                0,
                size_of::<UniformBufferObject>() as u64,
                MemoryMapFlags::empty(),
            )
        }?;

        unsafe { copy_nonoverlapping(&ubo, memory.cast(), 1) };

        unsafe {
            self.device
                .unmap_memory(self.data.uniform_buffers_memory[image_index])
        };

        Ok(())
    }

    unsafe fn create_descriptor_pool(device: &Device, data: &mut VulkanData) -> Result<()> {
        let ubo_size = DescriptorPoolSize::builder()
            .type_(DescriptorType::UNIFORM_BUFFER)
            .descriptor_count(data.swapchain_images.len() as u32);

        let pool_sizes = &[ubo_size];
        let info = DescriptorPoolCreateInfo::builder()
            .pool_sizes(pool_sizes)
            .max_sets(data.swapchain_images.len() as u32);

        data.descriptor_pool = unsafe { device.create_descriptor_pool(&info, None) }?;

        Ok(())
    }

    unsafe fn create_descriptor_sets(device: &Device, data: &mut VulkanData) -> Result<()> {
        let layouts = vec![data.descriptor_set_layout; data.swapchain_images.len()];
        let info = DescriptorSetAllocateInfo::builder()
            .descriptor_pool(data.descriptor_pool)
            .set_layouts(&layouts);

        data.descriptor_sets = unsafe { device.allocate_descriptor_sets(&info) }?;

        for i in 0..data.swapchain_images.len() {
            let info = DescriptorBufferInfo::builder()
                .buffer(data.uniform_buffers[i])
                .offset(0)
                .range(size_of::<UniformBufferObject>() as u64);

            let buffer_info = &[info];
            let ubo_write = WriteDescriptorSet::builder()
                .dst_set(data.descriptor_sets[i])
                .dst_binding(0)
                .dst_array_element(0)
                .descriptor_type(DescriptorType::UNIFORM_BUFFER)
                .buffer_info(buffer_info);

            unsafe { device.update_descriptor_sets(&[ubo_write], &[] as &[CopyDescriptorSet]) };
        }

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

        let flags = if cfg!(target_os = "macos") && entry.version()? >= PORTABILITY_MACOS_VERSION {
            info!("Enabling extensions for macOS portability");
            extensions.push(KHR_GET_PHYSICAL_DEVICE_PROPERTIES2_EXTENSION.name.as_ptr());
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

        let features = PhysicalDeviceFeatures::builder();

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

    unsafe fn create_swapchain_image_views(device: &Device, data: &mut VulkanData) -> Result<()> {
        data.swapchain_image_views = data
            .swapchain_images
            .iter()
            .map(|i| {
                let components = ComponentMapping::builder()
                    .r(ComponentSwizzle::IDENTITY)
                    .g(ComponentSwizzle::IDENTITY)
                    .b(ComponentSwizzle::IDENTITY)
                    .a(ComponentSwizzle::IDENTITY);

                let subresource_range = ImageSubresourceRange::builder()
                    .aspect_mask(ImageAspectFlags::COLOR)
                    .base_mip_level(0)
                    .level_count(1)
                    .base_array_layer(0)
                    .layer_count(1);

                let info = ImageViewCreateInfo::builder()
                    .image(*i)
                    .view_type(ImageViewType::_2D)
                    .format(data.swapchain_format)
                    .components(components)
                    .subresource_range(subresource_range);

                unsafe { device.create_image_view(&info, None) }
            })
            .collect::<Result<Vec<_>, _>>()?;
        Ok(())
    }

    unsafe fn create_descriptor_set_layout(device: &Device, data: &mut VulkanData) -> Result<()> {
        let ubo_binding = DescriptorSetLayoutBinding::builder()
            .binding(0)
            .descriptor_type(DescriptorType::UNIFORM_BUFFER)
            .descriptor_count(1)
            .stage_flags(ShaderStageFlags::VERTEX);

        let bindings = &[ubo_binding];
        let info = DescriptorSetLayoutCreateInfo::builder().bindings(bindings);

        data.descriptor_set_layout = unsafe { device.create_descriptor_set_layout(&info, None) }?;

        let set_layouts = &[data.descriptor_set_layout];
        let layout_info = PipelineLayoutCreateInfo::builder().set_layouts(set_layouts);

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

        let set_layouts = &[data.descriptor_set_layout];
        let layout_info = PipelineLayoutCreateInfo::builder().set_layouts(set_layouts);

        data.pipeline_layout = unsafe { device.create_pipeline_layout(&layout_info, None) }?;

        let stages = &[vert_stage, frag_stage];
        let info = GraphicsPipelineCreateInfo::builder()
            .stages(stages)
            .vertex_input_state(&vertex_input_state)
            .input_assembly_state(&input_assembly_state)
            .viewport_state(&viewport_state)
            .rasterization_state(&rasterization_state)
            .multisample_state(&multisample_state)
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
        let subpass = SubpassDescription::builder()
            .pipeline_bind_point(PipelineBindPoint::GRAPHICS)
            .color_attachments(color_attachments);

        let dependency = SubpassDependency::builder()
            .src_subpass(SUBPASS_EXTERNAL)
            .dst_subpass(0)
            .src_stage_mask(PipelineStageFlags::COLOR_ATTACHMENT_OUTPUT)
            .src_access_mask(AccessFlags::empty())
            .dst_stage_mask(PipelineStageFlags::COLOR_ATTACHMENT_OUTPUT)
            .dst_access_mask(AccessFlags::COLOR_ATTACHMENT_WRITE);

        let attachments = &[color_attachment];
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
                let attachments = &[*i];
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

    unsafe fn create_command_buffers(device: &Device, data: &mut VulkanData) -> Result<()> {
        let allocate_info = CommandBufferAllocateInfo::builder()
            .command_pool(data.command_pool)
            .level(CommandBufferLevel::PRIMARY)
            .command_buffer_count(data.framebuffers.len() as u32);

        data.command_buffers = unsafe { device.allocate_command_buffers(&allocate_info) }?;

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

            let clear_values = &[color_clear_value];
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
                device.cmd_bind_vertex_buffers(*command_buffer, 0, &[data.vertex_buffer], &[0]);
                device.cmd_bind_index_buffer(
                    *command_buffer,
                    data.index_buffer,
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

    unsafe fn recreate_swapchain(&mut self, window: &Window) -> Result<()> {
        debug!("Recreating Swapchain");
        unsafe {
            self.device.device_wait_idle()?;
            self.destroy_swapchain();
            Self::create_swapchain(window, &self.instance, &self.device, &mut self.data)?;
            Self::create_swapchain_image_views(&self.device, &mut self.data)?;
            Self::create_render_pass(&self.instance, &self.device, &mut self.data)?;
            Self::create_pipeline(&self.device, &mut self.data)?;
            Self::create_framebuffers(&self.device, &mut self.data)?;
            Self::create_uniform_buffers(&self.instance, &self.device, &mut self.data)?;
            Self::create_descriptor_pool(&self.device, &mut self.data)?;
            Self::create_descriptor_sets(&self.device, &mut self.data)?;
            Self::create_command_buffers(&self.device, &mut self.data)?;
        }
        self.data
            .images_in_flight
            .resize(self.data.swapchain_images.len(), Fence::null());
        Ok(())
    }

    unsafe fn create_uniform_buffers(
        instance: &Instance,
        device: &Device,
        data: &mut VulkanData,
    ) -> Result<()> {
        data.uniform_buffers.clear();
        data.uniform_buffers_memory.clear();

        for _ in 0..data.swapchain_images.len() {
            let (uniform_buffer, uniform_buffer_memory) = unsafe {
                Self::create_buffer(
                    instance,
                    device,
                    data,
                    size_of::<UniformBufferObject>() as u64,
                    BufferUsageFlags::UNIFORM_BUFFER,
                    MemoryPropertyFlags::HOST_COHERENT | MemoryPropertyFlags::HOST_VISIBLE,
                )
            }?;

            data.uniform_buffers.push(uniform_buffer);
            data.uniform_buffers_memory.push(uniform_buffer_memory);
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
                Self::get_memory_type_index(instance, data, properties, requirements)
            }?);

        let buffer_memory = unsafe { device.allocate_memory(&memory_info, None) }?;

        (unsafe { device.bind_buffer_memory(buffer, buffer_memory, 0) })?;

        Ok((buffer, buffer_memory))
    }

    unsafe fn create_vertex_buffer(
        instance: &Instance,
        device: &Device,
        data: &mut VulkanData,
    ) -> Result<()> {
        let size = (size_of::<Vertex>() * VERTICES.len()) as u64;

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

        unsafe { copy_nonoverlapping(VERTICES.as_ptr(), memory.cast(), VERTICES.len()) };

        unsafe { device.unmap_memory(staging_buffer_memory) };

        let (vertex_buffer, vertex_buffer_memory) = unsafe {
            Self::create_buffer(
                instance,
                device,
                data,
                size,
                BufferUsageFlags::VERTEX_BUFFER | BufferUsageFlags::TRANSFER_DST,
                MemoryPropertyFlags::DEVICE_LOCAL,
            )
        }?;

        data.vertex_buffer = vertex_buffer;
        data.vertex_buffer_memory = vertex_buffer_memory;

        (unsafe { Self::copy_buffer(device, data, staging_buffer, vertex_buffer, size) })?;

        unsafe { device.destroy_buffer(staging_buffer, None) };
        unsafe { device.free_memory(staging_buffer_memory, None) };

        Ok(())
    }

    unsafe fn create_index_buffer(
        instance: &Instance,
        device: &Device,
        data: &mut VulkanData,
    ) -> Result<()> {
        let size = (size_of::<u16>() * INDICES.len()) as u64;

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

        unsafe { copy_nonoverlapping(INDICES.as_ptr(), memory.cast(), INDICES.len()) };

        unsafe { device.unmap_memory(staging_buffer_memory) };

        let (index_buffer, index_buffer_memory) = unsafe {
            Self::create_buffer(
                instance,
                device,
                data,
                size,
                BufferUsageFlags::TRANSFER_DST | BufferUsageFlags::INDEX_BUFFER,
                MemoryPropertyFlags::DEVICE_LOCAL,
            )
        }?;

        data.index_buffer = index_buffer;
        data.index_buffer_memory = index_buffer_memory;

        (unsafe { Self::copy_buffer(device, data, staging_buffer, index_buffer, size) })?;

        unsafe { device.destroy_buffer(staging_buffer, None) };
        unsafe { device.free_memory(staging_buffer_memory, None) };

        Ok(())
    }

    unsafe fn copy_buffer(
        device: &Device,
        data: &VulkanData,
        source: Buffer,
        destination: Buffer,
        size: DeviceSize,
    ) -> Result<()> {
        let info = CommandBufferAllocateInfo::builder()
            .level(CommandBufferLevel::PRIMARY)
            .command_pool(data.command_pool)
            .command_buffer_count(1);

        let command_buffer = unsafe { device.allocate_command_buffers(&info) }?[0];

        let info =
            CommandBufferBeginInfo::builder().flags(CommandBufferUsageFlags::ONE_TIME_SUBMIT);

        (unsafe { device.begin_command_buffer(command_buffer, &info) })?;

        let regions = BufferCopy::builder().size(size);
        unsafe { device.cmd_copy_buffer(command_buffer, source, destination, &[regions]) };

        (unsafe { device.end_command_buffer(command_buffer) })?;

        let command_buffers = &[command_buffer];
        let info = SubmitInfo::builder().command_buffers(command_buffers);

        (unsafe { device.queue_submit(data.graphics_queue, &[info], Fence::null()) })?;
        (unsafe { device.queue_wait_idle(data.graphics_queue) })?;
        unsafe { device.free_command_buffers(data.command_pool, &[command_buffer]) };

        Ok(())
    }

    unsafe fn get_memory_type_index(
        instance: &Instance,
        data: &VulkanData,
        properties: MemoryPropertyFlags,
        requirements: MemoryRequirements,
    ) -> Result<u32> {
        let memory =
            unsafe { instance.get_physical_device_memory_properties(data.physical_device) };
        (0..memory.memory_type_count)
            .find(|i| {
                let suitable = (requirements.memory_type_bits & (1 << i)) != 0;
                let memory_type = memory.memory_types[*i as usize];
                suitable && memory_type.property_flags.contains(properties)
            })
            .ok_or_else(|| anyhow!("Failed to find suitable memory type."))
    }

    unsafe fn destroy_swapchain(&mut self) {
        debug!("Destroying Swapchain");
        unsafe {
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
        unsafe {
            self.data
                .uniform_buffers
                .iter()
                .for_each(|b| self.device.destroy_buffer(*b, None));
            self.data
                .uniform_buffers_memory
                .iter()
                .for_each(|m| self.device.free_memory(*m, None));
            self.device.device_wait_idle().unwrap();

            self.destroy_swapchain();
            self.device
                .destroy_descriptor_set_layout(self.data.descriptor_set_layout, None);
            self.device.destroy_buffer(self.data.index_buffer, None);
            self.device.free_memory(self.data.index_buffer_memory, None);
            self.device.destroy_buffer(self.data.vertex_buffer, None);
            self.device
                .free_memory(self.data.vertex_buffer_memory, None);

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
        for (index, properties) in properties.iter().enumerate() {
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
