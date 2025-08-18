use crate::engine::Engine;
use crate::init::Init;
use anyhow::Result;
use std::sync::Arc;
use vulkano::VulkanLibrary;
use vulkano::device::Device;
use vulkano::device::physical::PhysicalDevice;
use vulkano::instance::{Instance, InstanceCreateInfo};
use vulkano::swapchain::Surface;

/// Contains the main Vulkan code of the rendering engine
pub struct VulkanProperties {
    library: Arc<VulkanLibrary>,

    instance: Init<Arc<Instance>>,
    physical_device: Init<Arc<PhysicalDevice>>,
    device: Init<Device>,
    surface: Init<Surface>,
}

impl VulkanProperties {
    pub fn new() -> Result<Self> {
        Ok(Self {
            library: VulkanLibrary::new()?,

            instance: Init::uninit(),
            physical_device: Init::uninit(),
            device: Init::uninit(),
            surface: Init::uninit(),
        })
    }

    /// Tries to pick the best physical device
    fn pick_best_physical_device(&mut self) -> Arc<PhysicalDevice> {
        let mut physical_devices = self
            .instance
            .enumerate_physical_devices()
            .unwrap()
            .into_iter()
            .map(|physical_device| {
                let mut fitness = 0u8;

                let properties = physical_device.properties();
                let extension_properties = physical_device.extension_properties();

                println!("Extension properties: {extension_properties:#?}");

                (physical_device, fitness)
            })
            .collect::<Vec<(Arc<PhysicalDevice>, u8)>>();

        physical_devices.sort_by(|(_, x), (_, y)| x.cmp(y));

        physical_devices[0].0.clone()
    }

    /// Initializes the vulkan library from a blank VulkanProperties
    pub fn init_vulkan<S: ToString>(&mut self, application_name: S) -> Result<()> {
        *self.instance = Instance::new(
            self.library.clone(),
            InstanceCreateInfo {
                application_name: Some(application_name.to_string()),
                ..Default::default()
            },
        )?;

        *self.physical_device = self.pick_best_physical_device();

        Ok(())
    }

    pub fn init_swapchain(&mut self) {}

    pub fn init_commands(&mut self) {}

    pub fn init_sync_structures(&mut self) {}
}
