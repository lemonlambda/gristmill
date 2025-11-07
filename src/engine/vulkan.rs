use anyhow::{Result, anyhow};
use vulkanalia::{
    Entry, Instance,
    loader::LIBRARY,
    loader::LibloadingLoader,
    vk::{ApplicationInfo, HasBuilder, InstanceCreateInfo, InstanceV1_0, make_version},
    window::get_required_instance_extensions,
};
use winit::window::Window;

#[derive(Clone, Debug)]
pub struct VulkanApp {
    entry: Entry,
    instance: Instance,
}

impl VulkanApp {
    pub unsafe fn create(window: &Window) -> Result<Self> {
        let loader = LibloadingLoader::new(LIBRARY)?;
        let entry = unsafe { Entry::new(loader).map_err(|b| anyhow!("{}", b))? };
        let instance = unsafe { Self::create_instance(window, &entry) }?;
        Ok(Self { entry, instance })
    }

    pub unsafe fn create_instance(window: &Window, entry: &Entry) -> Result<Instance> {
        let application_info = ApplicationInfo::builder()
            .application_name(b"Factory Game\0")
            .application_version(make_version(1, 0, 0))
            .engine_name(b"Lemgine\0")
            .engine_version(make_version(1, 0, 0))
            .api_version(make_version(1, 0, 0));

        let extensions = get_required_instance_extensions(window)
            .iter()
            .map(|e| e.as_ptr())
            .collect::<Vec<_>>();

        let info = InstanceCreateInfo::builder()
            .application_info(&application_info)
            .enabled_extension_names(&extensions);

        Ok(unsafe { entry.create_instance(&info, None) }?)
    }

    pub unsafe fn render(&mut self, window: &Window) -> Result<()> {
        Ok(())
    }

    pub unsafe fn destroy(&mut self) {
        unsafe { self.instance.destroy_instance(None) }
    }
}

#[derive(Clone, Debug, Default)]
pub struct VulkanData {}
