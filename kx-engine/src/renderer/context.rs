use anyhow::{Context, Result};
use ash::{ext, khr, vk};
use raw_window_handle::{RawDisplayHandle, RawWindowHandle};

pub struct QueueFamilyIndices {
    pub graphics: u32,
    pub present: u32,
}

pub struct VulkanContext {
    entry: ash::Entry,
    pub instance: ash::Instance,
    pub surface: vk::SurfaceKHR,
    pub surface_fn: khr::surface::Instance,
    pub physical_device: vk::PhysicalDevice,
    pub device: ash::Device,
    pub queue_families: QueueFamilyIndices,
    pub graphics_queue: vk::Queue,
    pub present_queue: vk::Queue,
}

impl VulkanContext {
    pub fn new(display_handle: RawDisplayHandle, window_handle: RawWindowHandle) -> Result<Self> {
        let entry = ash::Entry::linked();

        let app_info = vk::ApplicationInfo::default()
            .application_name(c"kx")
            .application_version(vk::make_api_version(0, 0, 1, 0))
            .engine_name(c"kx-engine")
            .engine_version(vk::make_api_version(0, 0, 1, 0))
            .api_version(vk::make_api_version(0, 1, 4, 0));

        let mut extensions = ash_window::enumerate_required_extensions(display_handle)?.to_vec();

        #[cfg(debug_assertions)]
        extensions.push(ext::debug_utils::NAME.as_ptr());

        let mut layers: Vec<*const i8> = Vec::new();

        #[cfg(debug_assertions)]
        layers.push(c"VK_LAYER_KHRONOS_validation".as_ptr());

        let instance = {
            let create_info = vk::InstanceCreateInfo::default()
                .application_info(&app_info)
                .enabled_extension_names(&extensions)
                .enabled_layer_names(&layers);

            unsafe { entry.create_instance(&create_info, None)? }
        };

        let surface = unsafe {
            ash_window::create_surface(&entry, &instance, display_handle, window_handle, None)?
        };

        let surface_fn = khr::surface::Instance::new(&entry, &instance);

        let (physical_device, queue_families) =
            pick_physical_device(&instance, surface, &surface_fn)?;

        let unique_families: Vec<u32> = if queue_families.graphics == queue_families.present {
            vec![queue_families.graphics]
        } else {
            vec![queue_families.graphics, queue_families.present]
        };

        let queue_priority = [1.0];

        let device = {
            let queue_create_infos: Vec<vk::DeviceQueueCreateInfo> = unique_families
                .iter()
                .map(|&index| {
                    vk::DeviceQueueCreateInfo::default()
                        .queue_family_index(index)
                        .queue_priorities(&queue_priority)
                })
                .collect();

            const DEVICE_EXTENSIONS: [*const i8; 1] = [khr::swapchain::NAME.as_ptr()];

            let create_info = vk::DeviceCreateInfo::default()
                .queue_create_infos(&queue_create_infos)
                .enabled_extension_names(&DEVICE_EXTENSIONS);

            unsafe { instance.create_device(physical_device, &create_info, None)? }
        };

        let graphics_queue = unsafe { device.get_device_queue(queue_families.graphics, 0) };
        let present_queue = unsafe { device.get_device_queue(queue_families.present, 0) };

        Ok(Self {
            entry,
            instance,
            surface,
            surface_fn,
            physical_device,
            device,
            queue_families,
            graphics_queue,
            present_queue,
        })
    }
}

fn pick_physical_device(
    instance: &ash::Instance,
    surface: vk::SurfaceKHR,
    surface_fn: &khr::surface::Instance,
) -> Result<(vk::PhysicalDevice, QueueFamilyIndices)> {
    let devices = unsafe { instance.enumerate_physical_devices()? };

    let mut best: Option<(vk::PhysicalDevice, QueueFamilyIndices, bool)> = None;

    for &physical_device in &devices {
        if let Some(indices) = find_queue_families(instance, surface, surface_fn, physical_device) {
            let props = unsafe { instance.get_physical_device_properties(physical_device) };
            let is_discrete = props.device_type == vk::PhysicalDeviceType::DISCRETE_GPU;

            if is_discrete {
                return Ok((physical_device, indices));
            }

            if best.is_none() {
                best = Some((physical_device, indices, is_discrete));
            }
        }
    }

    best.map(|(physical_device, indices, _)| (physical_device, indices))
        .context("no suitable physical device found")
}

fn find_queue_families(
    instance: &ash::Instance,
    surface: vk::SurfaceKHR,
    surface_fn: &khr::surface::Instance,
    physical_device: vk::PhysicalDevice,
) -> Option<QueueFamilyIndices> {
    let families = unsafe { instance.get_physical_device_queue_family_properties(physical_device) };

    let mut graphics = None;
    let mut present = None;

    for (i, family) in families.iter().enumerate() {
        let i = i as u32;

        if family.queue_flags.contains(vk::QueueFlags::GRAPHICS) {
            graphics = Some(i);
        }

        let present_support = unsafe {
            surface_fn
                .get_physical_device_surface_support(physical_device, i, surface)
                .unwrap_or(false)
        };

        if present_support {
            present = Some(i);
        }

        if graphics == present && graphics.is_some() {
            break;
        }
    }

    Some(QueueFamilyIndices {
        graphics: graphics?,
        present: present?,
    })
}

impl Drop for VulkanContext {
    fn drop(&mut self) {
        unsafe {
            self.device.destroy_device(None);
            self.surface_fn.destroy_surface(self.surface, None);
            self.instance.destroy_instance(None);
        }
    }
}
