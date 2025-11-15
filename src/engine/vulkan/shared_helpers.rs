use anyhow::{Result, anyhow};
use vulkanalia::Instance;
use vulkanalia::vk::*;

use crate::engine::vulkan::VulkanData;

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
