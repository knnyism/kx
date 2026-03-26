use anyhow::{Result, bail};
use std::ffi::{CStr, CString, c_void};

use ash::vk;

use super::Instance;

const _: () = assert!(size_of::<*mut c_void>() == 8, "assumes 64-bit pointers");
const FEATURE_HEADER_SIZE: usize = 16;

#[derive(Clone)]
pub(crate) struct GenericFeature {
    bytes: Vec<u8>,
}

impl GenericFeature {
    fn new<F: vk::ExtendsPhysicalDeviceFeatures2>(feature: F) -> Self {
        let size = size_of::<F>();
        let bytes = unsafe { std::slice::from_raw_parts(&feature as *const F as *const u8, size) };
        Self {
            bytes: bytes.to_vec(),
        }
    }

    fn zeroed_query_copy(&self) -> Self {
        let mut copy = Self {
            bytes: vec![0u8; self.bytes.len()],
        };
        copy.bytes[..4].copy_from_slice(&self.bytes[..4]);
        copy
    }

    pub(crate) fn as_mut_ptr(&mut self) -> *mut c_void {
        self.bytes.as_mut_ptr() as *mut c_void
    }

    pub(crate) fn set_p_next(&mut self, next: *mut c_void) {
        let ptr_bytes = (next as usize).to_ne_bytes();
        self.bytes[8..16].copy_from_slice(&ptr_bytes);
    }

    fn satisfied_by(&self, supported: &GenericFeature) -> bool {
        debug_assert_eq!(self.bytes[..4], supported.bytes[..4], "sType mismatch");
        debug_assert_eq!(self.bytes.len(), supported.bytes.len(), "size mismatch");

        let fields = &self.bytes[FEATURE_HEADER_SIZE..];
        let supported_fields = &supported.bytes[FEATURE_HEADER_SIZE..];

        for chunk in (0..fields.len()).step_by(4) {
            let end = (chunk + 4).min(fields.len());
            if end - chunk < 4 {
                break;
            }
            let req = u32::from_ne_bytes(fields[chunk..end].try_into().unwrap());
            let sup = u32::from_ne_bytes(supported_fields[chunk..end].try_into().unwrap());
            if req == vk::TRUE && sup != vk::TRUE {
                return false;
            }
        }

        true
    }
}

pub(crate) fn chain_features(
    features2: &mut vk::PhysicalDeviceFeatures2,
    chain: &mut [GenericFeature],
) {
    let mut prev: *mut c_void = std::ptr::null_mut();
    for node in chain.iter_mut().rev() {
        node.set_p_next(prev);
        prev = node.as_mut_ptr();
    }
    features2.p_next = prev;
}

pub struct PhysicalDevice {
    pub(crate) physical_device: vk::PhysicalDevice,
    pub(crate) features: Vec<GenericFeature>,
    pub(crate) extensions: Vec<CString>,
}

impl PhysicalDevice {
    pub fn handle(&self) -> vk::PhysicalDevice {
        self.physical_device
    }
}

pub struct PhysicalDeviceSelector<'a> {
    instance: &'a Instance,
    preferred_type: vk::PhysicalDeviceType,
    required_extensions: Vec<&'a CStr>,
    required_features: Vec<GenericFeature>,
}

impl<'a> PhysicalDeviceSelector<'a> {
    pub fn new(instance: &'a Instance) -> Self {
        Self {
            instance,
            preferred_type: vk::PhysicalDeviceType::DISCRETE_GPU,
            required_extensions: Vec::new(),
            required_features: Vec::new(),
        }
    }

    pub fn prefer_type(mut self, device_type: vk::PhysicalDeviceType) -> Self {
        self.preferred_type = device_type;
        self
    }

    pub fn require_extension(mut self, extension: &'a CStr) -> Self {
        self.required_extensions.push(extension);
        self
    }

    pub fn add_required_extension_feature<F: vk::ExtendsPhysicalDeviceFeatures2>(
        mut self,
        feature: F,
    ) -> Self {
        self.required_features.push(GenericFeature::new(feature));
        self
    }

    pub fn select(self) -> Result<PhysicalDevice> {
        let devices = unsafe { self.instance.enumerate_physical_devices()? };
        if devices.is_empty() {
            bail!("no Vulkan physical devices found");
        }

        let mut best: Option<(vk::PhysicalDevice, bool)> = None;

        for pd in devices {
            if !self.has_required_extensions(pd)? {
                continue;
            }
            if !self.has_present_support(pd)? {
                continue;
            }
            if self.find_graphics_family(pd).is_none() {
                continue;
            }
            if !self.has_required_features(pd) {
                continue;
            }

            let props = unsafe { self.instance.get_physical_device_properties(pd) };
            let is_preferred = props.device_type == self.preferred_type;

            match &best {
                Some((_, true)) if !is_preferred => {}
                _ if is_preferred => best = Some((pd, true)),
                None => best = Some((pd, false)),
                _ => {}
            }
        }

        let handle = best
            .map(|(pd, _)| pd)
            .ok_or_else(|| anyhow::anyhow!("no suitable physical device found"))?;

        let extensions = self
            .required_extensions
            .iter()
            .map(|e| CString::from(*e))
            .collect();

        Ok(PhysicalDevice {
            physical_device: handle,
            features: self.required_features,
            extensions,
        })
    }

    fn has_required_extensions(&self, pd: vk::PhysicalDevice) -> Result<bool> {
        if self.required_extensions.is_empty() {
            return Ok(true);
        }

        let available = unsafe { self.instance.enumerate_device_extension_properties(pd)? };

        for required in &self.required_extensions {
            let found = available.iter().any(|ext| {
                ext.extension_name_as_c_str()
                    .map(|name| name == *required)
                    .unwrap_or(false)
            });
            if !found {
                return Ok(false);
            }
        }

        Ok(true)
    }

    fn has_present_support(&self, pd: vk::PhysicalDevice) -> Result<bool> {
        let families = unsafe {
            self.instance
                .get_physical_device_queue_family_properties(pd)
        };

        for (i, _) in families.iter().enumerate() {
            let supported = unsafe {
                self.instance
                    .surface_fn()
                    .get_physical_device_surface_support(pd, i as u32, self.instance.surface())?
            };
            if supported {
                return Ok(true);
            }
        }

        Ok(false)
    }

    fn has_required_features(&self, pd: vk::PhysicalDevice) -> bool {
        if self.required_features.is_empty() {
            return true;
        }

        let mut supported: Vec<GenericFeature> = self
            .required_features
            .iter()
            .map(|r| r.zeroed_query_copy())
            .collect();

        let mut features2 = vk::PhysicalDeviceFeatures2::default();
        chain_features(&mut features2, &mut supported);

        unsafe {
            self.instance
                .get_physical_device_features2(pd, &mut features2)
        };

        self.required_features
            .iter()
            .zip(supported.iter())
            .all(|(req, sup)| req.satisfied_by(sup))
    }

    fn find_graphics_family(&self, pd: vk::PhysicalDevice) -> Option<u32> {
        let families = unsafe {
            self.instance
                .get_physical_device_queue_family_properties(pd)
        };

        families
            .iter()
            .position(|f| f.queue_flags.contains(vk::QueueFlags::GRAPHICS))
            .map(|i| i as u32)
    }
}

pub struct Device {
    device: ash::Device,
    physical_device: vk::PhysicalDevice,
    graphics_family: u32,
    present_family: u32,
}

impl Device {
    pub fn physical_device(&self) -> vk::PhysicalDevice {
        self.physical_device
    }

    pub fn graphics_family(&self) -> u32 {
        self.graphics_family
    }

    pub fn present_family(&self) -> u32 {
        self.present_family
    }

    pub fn destroy(&self) {
        unsafe { self.device.destroy_device(None) };
    }
}

impl std::ops::Deref for Device {
    type Target = ash::Device;
    fn deref(&self) -> &Self::Target {
        &self.device
    }
}

pub struct DeviceBuilder<'a> {
    instance: &'a Instance,
    selected: PhysicalDevice,
}

impl<'a> DeviceBuilder<'a> {
    pub fn new(instance: &'a Instance, selected: PhysicalDevice) -> Self {
        Self { instance, selected }
    }

    pub fn build(mut self) -> Result<(Device, vk::Queue, vk::Queue)> {
        let families = unsafe {
            self.instance
                .get_physical_device_queue_family_properties(self.selected.physical_device)
        };

        let graphics_family = families
            .iter()
            .position(|f| f.queue_flags.contains(vk::QueueFlags::GRAPHICS))
            .ok_or_else(|| anyhow::anyhow!("no graphics queue family"))?
            as u32;

        let present_family = {
            let mut found = None;
            for (i, _) in families.iter().enumerate() {
                let supported = unsafe {
                    self.instance
                        .surface_fn()
                        .get_physical_device_surface_support(
                            self.selected.physical_device,
                            i as u32,
                            self.instance.surface(),
                        )?
                };
                if supported {
                    found = Some(i as u32);
                    break;
                }
            }
            found.ok_or_else(|| anyhow::anyhow!("no present queue family"))?
        };

        let unique_families: Vec<u32> = if graphics_family == present_family {
            vec![graphics_family]
        } else {
            vec![graphics_family, present_family]
        };

        let priorities = [1.0f32];
        let queue_create_infos: Vec<vk::DeviceQueueCreateInfo> = unique_families
            .iter()
            .map(|&family| {
                vk::DeviceQueueCreateInfo::default()
                    .queue_family_index(family)
                    .queue_priorities(&priorities)
            })
            .collect();

        let extension_ptrs: Vec<*const _> = self
            .selected
            .extensions
            .iter()
            .map(|e| e.as_ptr())
            .collect();

        let mut features2 = vk::PhysicalDeviceFeatures2::default();
        chain_features(&mut features2, &mut self.selected.features);

        let create_info = vk::DeviceCreateInfo::default()
            .queue_create_infos(&queue_create_infos)
            .enabled_extension_names(&extension_ptrs)
            .push_next(&mut features2);

        let device = unsafe {
            self.instance
                .create_device(self.selected.physical_device, &create_info, None)?
        };

        let graphics_queue = unsafe { device.get_device_queue(graphics_family, 0) };
        let present_queue = unsafe { device.get_device_queue(present_family, 0) };

        Ok((
            Device {
                device,
                physical_device: self.selected.physical_device,
                graphics_family,
                present_family,
            },
            graphics_queue,
            present_queue,
        ))
    }
}
