use crate::engine::Engine;
use crate::init::Init;
use anyhow::Result;
use std::sync::Arc;
use vulkano::VulkanLibrary;
use vulkano::device::Device;
use vulkano::device::physical::PhysicalDevice;
use vulkano::instance::{Instance, InstanceCreateInfo};
use vulkano::swapchain::Surface;

pub struct VulkanProperties {
    library: Arc<VulkanLibrary>,

    instance: Init<Arc<Instance>>,
    physical_device: Init<PhysicalDevice>,
    device: Init<Device>,
    surface: Init<Surface>,
}

impl VulkanProperties {
    pub fn new() -> Result<Self> {
        Ok(Self {
            library: VulkanLibrary::new()?,

            instance: Init::blank(),
            physical_device: Init::blank(),
            device: Init::blank(),
            surface: Init::blank(),
        })
    }

    pub fn init_vulkan(&mut self) -> Result<()> {
        *self.instance = Instance::new(
            self.library.clone(),
            InstanceCreateInfo {
                application_name: Some(String::from("Tringl")),
                ..Default::default()
            },
        )?;

        Ok(())
    }

    pub fn init_swapchain(&mut self) {}

    pub fn init_commands(&mut self) {}

    pub fn init_sync_structures(&mut self) {}
}
