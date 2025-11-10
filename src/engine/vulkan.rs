use anyhow::{Result, anyhow};
use log::*;
use std::{
    collections::HashSet,
    ffi::{CStr, c_void},
};
use thiserror::Error;
use vulkanalia::{
    Entry, Instance, Version,
    loader::{LIBRARY, LibloadingLoader},
    vk::{
        ApplicationInfo, Bool32, DebugUtilsMessageSeverityFlagsEXT, DebugUtilsMessageTypeFlagsEXT,
        DebugUtilsMessengerCallbackDataEXT, DebugUtilsMessengerCreateInfoEXT,
        DebugUtilsMessengerEXT, EXT_DEBUG_UTILS_EXTENSION, EntryV1_0,
        ExtDebugUtilsExtensionInstanceCommands, ExtensionName, FALSE, HasBuilder,
        InstanceCreateFlags, InstanceCreateInfo, InstanceV1_0,
        KHR_GET_PHYSICAL_DEVICE_PROPERTIES2_EXTENSION, KHR_PORTABILITY_ENUMERATION_EXTENSION,
        PhysicalDevice, PhysicalDeviceType, QueueFlags, TRUE, make_version,
    },
    window::get_required_instance_extensions,
};
use winit::window::Window;

const PORTABILITY_MACOS_VERSION: Version = Version::new(1, 3, 216);

const VALIDATION_ENABLED: bool = cfg!(debug_assertions);
const VALIDATION_LAYER: ExtensionName = ExtensionName::from_bytes(b"VK_LAYER_KHRONOS_validation");

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
#[error("Missing {0}.")]
pub struct SuitabilityError(pub &'static str);

#[derive(Clone, Debug)]
pub struct VulkanApp {
    entry: Entry,
    instance: Instance,
    data: VulkanData,
}

#[derive(Clone, Debug, Default)]
pub struct VulkanData {
    messenger: DebugUtilsMessengerEXT,
    physical_device: PhysicalDevice,
}

impl VulkanApp {
    pub unsafe fn create(window: &Window) -> Result<Self> {
        let loader = unsafe { LibloadingLoader::new(LIBRARY)? };
        let entry = unsafe { Entry::new(loader).map_err(|b| anyhow!("{}", b))? };
        let mut data = VulkanData::default();
        let instance = unsafe { Self::create_instance(window, &entry, &mut data) }?;
        unsafe { Self::pick_physical_device(&instance, &mut data)? };
        Ok(Self {
            entry,
            instance,
            data,
        })
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

        if properties.device_type != PhysicalDeviceType::DISCRETE_GPU {
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

        Ok(())
    }

    pub unsafe fn render(&mut self, window: &Window) -> Result<()> {
        Ok(())
    }

    pub unsafe fn destroy(&mut self) {
        if VALIDATION_ENABLED {
            unsafe {
                self.instance
                    .destroy_debug_utils_messenger_ext(self.data.messenger, None)
            };
        }

        unsafe { self.instance.destroy_instance(None) }
    }
}

#[derive(Copy, Clone, Debug)]
struct QueueFamilyIndices {
    graphics: u32,
}

impl QueueFamilyIndices {
    unsafe fn get(
        instance: &Instance,
        data: &VulkanData,
        physical_device: PhysicalDevice,
    ) -> Result<Self> {
        let properties = instance.get_physical_device_queue_family_properties(physical_device);

        let graphics = properties
            .iter()
            .position(|p| p.queue_flags.contains(QueueFlags::GRAPHICS))
            .map(|i| i as u32);

        if let Some(graphics) = graphics {
            Ok(Self { graphics })
        } else {
            Err(anyhow!(SuitabilityError(
                "Missing required queue families."
            )))
        }
    }
}
