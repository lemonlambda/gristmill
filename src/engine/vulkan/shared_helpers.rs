use anyhow::{Result, anyhow};
use log::*;
use vulkanalia::vk::*;
use vulkanalia::{Device, Instance};

use crate::engine::vertex::Vertex;
use crate::engine::vulkan::VulkanData;
use crate::engine::vulkan::buffer_manager::buffer_pair::StandardBufferMaps;
use crate::engine::vulkan::buffer_manager::{
    AllocateBufferType, BufferManager, BufferManagerCopyType, BufferManagerDataType,
    BufferManagerRequirements,
};

pub unsafe fn get_memory_type_index(
    instance: &Instance,
    physical_device: PhysicalDevice,
    properties: MemoryPropertyFlags,
    requirements: MemoryRequirements,
) -> Result<u32> {
    let memory = unsafe { instance.get_physical_device_memory_properties(physical_device) };
    (0..memory.memory_type_count)
        .find(|i| {
            let suitable = (requirements.memory_type_bits & (1 << i)) != 0;
            let memory_type = memory.memory_types[*i as usize];
            suitable && memory_type.property_flags.contains(properties)
        })
        .ok_or_else(|| anyhow!("Failed to find suitable memory type."))
}

pub unsafe fn begin_single_time_commands(
    device: &Device,
    command_pool: CommandPool,
) -> Result<CommandBuffer> {
    let info = CommandBufferAllocateInfo::builder()
        .level(CommandBufferLevel::PRIMARY)
        .command_pool(command_pool)
        .command_buffer_count(1);

    let command_buffer = unsafe { device.allocate_command_buffers(&info) }?[0];

    let info = CommandBufferBeginInfo::builder().flags(CommandBufferUsageFlags::ONE_TIME_SUBMIT);

    (unsafe { device.begin_command_buffer(command_buffer, &info) })?;

    Ok(command_buffer)
}

pub unsafe fn end_single_time_commands(
    device: &Device,
    graphics_queue: Queue,
    command_pool: CommandPool,
    command_buffer: CommandBuffer,
) -> Result<()> {
    (unsafe { device.end_command_buffer(command_buffer) })?;

    let command_buffers = &[command_buffer];
    let info = SubmitInfo::builder().command_buffers(command_buffers);

    (unsafe { device.queue_submit(graphics_queue, &[info], Fence::null()) })?;
    (unsafe { device.queue_wait_idle(graphics_queue) })?;

    unsafe { device.free_command_buffers(command_pool, &[command_buffer]) };

    Ok(())
}

pub unsafe fn copy_buffer(
    device: &Device,
    graphics_queue: Queue,
    command_pool: CommandPool,
    source: Buffer,
    destination: Buffer,
    size: DeviceSize,
) -> Result<()> {
    debug!("Copying to a buffer with size: {size}");
    let command_buffer = unsafe { begin_single_time_commands(device, command_pool) }?;

    let regions = BufferCopy::builder().size(size);
    unsafe { device.cmd_copy_buffer(command_buffer, source, destination, &[regions]) };

    (unsafe { end_single_time_commands(device, graphics_queue, command_pool, command_buffer) })?;

    Ok(())
}

pub unsafe fn create_vertex_buffer(
    data: &mut VulkanData,
    name: StandardBufferMaps,
    vertices: Vec<Vertex>,
) -> Result<()> {
    unsafe {
        let buffer_size = (size_of::<Vertex>() * vertices.len()) as u64;

        data.buffer_manager.allocate_buffer_with_size(
            AllocateBufferType::Temp,
            BufferUsageFlags::TRANSFER_SRC,
            MemoryPropertyFlags::HOST_COHERENT | MemoryPropertyFlags::HOST_VISIBLE,
            buffer_size,
        )?;

        data.buffer_manager.copy_data_to_buffer(
            BufferManagerDataType::Data(&vertices),
            BufferManagerCopyType::TempBuffer,
        )?;

        data.buffer_manager.allocate_buffer_with_size(
            AllocateBufferType::Standard { name },
            BufferUsageFlags::VERTEX_BUFFER | BufferUsageFlags::TRANSFER_DST,
            MemoryPropertyFlags::DEVICE_LOCAL,
            buffer_size,
        )?;

        data.buffer_manager
            .copy_data_to_buffer_with_size::<Vertex>(
                BufferManagerDataType::TempBuffer {
                    graphics_queue: data.graphics_queue,
                    command_pool: data.command_pool,
                },
                BufferManagerCopyType::StandardBuffer(name),
                buffer_size,
            )?;

        data.buffer_manager.free_temp_buffer()
    };

    Ok(())
}

pub unsafe fn create_index_buffer(
    data: &mut VulkanData,
    name: StandardBufferMaps,
    indices: Vec<u16>,
) -> Result<()> {
    unsafe {
        let index_size = (16 * indices.len()) as u64;

        data.buffer_manager.allocate_buffer_with_size(
            AllocateBufferType::Temp,
            BufferUsageFlags::TRANSFER_SRC,
            MemoryPropertyFlags::HOST_COHERENT | MemoryPropertyFlags::HOST_VISIBLE,
            index_size,
        )?;

        data.buffer_manager.copy_data_to_buffer(
            BufferManagerDataType::Data(&indices),
            BufferManagerCopyType::TempBuffer,
        )?;

        data.buffer_manager.allocate_buffer_with_size(
            AllocateBufferType::Standard { name },
            BufferUsageFlags::INDEX_BUFFER | BufferUsageFlags::TRANSFER_DST,
            MemoryPropertyFlags::DEVICE_LOCAL,
            index_size,
        )?;

        data.buffer_manager.copy_data_to_buffer_with_size::<u16>(
            BufferManagerDataType::TempBuffer {
                graphics_queue: data.graphics_queue,
                command_pool: data.command_pool,
            },
            BufferManagerCopyType::StandardBuffer(name),
            index_size,
        )?;

        data.buffer_manager.free_temp_buffer()
    };

    Ok(())
}
