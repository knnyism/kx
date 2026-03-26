use anyhow::Result;

use ash::{khr, vk};

use super::{Device, Instance};

pub struct Swapchain {
    swapchain: vk::SwapchainKHR,
    swapchain_fn: khr::swapchain::Device,
    pub format: vk::Format,
    pub extent: vk::Extent2D,
}

impl Swapchain {
    pub fn acquire_next_image(
        &self,
        timeout: u64,
        semaphore: vk::Semaphore,
        fence: vk::Fence,
    ) -> std::result::Result<(u32, bool), vk::Result> {
        unsafe {
            self.swapchain_fn
                .acquire_next_image(self.swapchain, timeout, semaphore, fence)
        }
    }

    pub fn present(
        &self,
        queue: vk::Queue,
        info: &vk::PresentInfoKHR,
    ) -> std::result::Result<bool, vk::Result> {
        unsafe { self.swapchain_fn.queue_present(queue, info) }
    }

    pub fn destroy(&self) {
        unsafe { self.swapchain_fn.destroy_swapchain(self.swapchain, None) };
    }
}

impl AsRef<vk::SwapchainKHR> for Swapchain {
    fn as_ref(&self) -> &vk::SwapchainKHR {
        &self.swapchain
    }
}

pub struct SwapchainBuilder<'a> {
    instance: &'a Instance,
    device: &'a Device,
    width: u32,
    height: u32,
    image_usage: vk::ImageUsageFlags,
    present_mode: Option<vk::PresentModeKHR>,
    format: Option<vk::Format>,
    color_space: Option<vk::ColorSpaceKHR>,
    old: vk::SwapchainKHR,
}

impl<'a> SwapchainBuilder<'a> {
    pub fn new(instance: &'a Instance, device: &'a Device) -> Self {
        Self {
            instance,
            device,
            width: 256,
            height: 256,
            image_usage: vk::ImageUsageFlags::COLOR_ATTACHMENT,
            present_mode: None,
            format: None,
            color_space: None,
            old: vk::SwapchainKHR::null(),
        }
    }

    pub fn size(mut self, width: u32, height: u32) -> Self {
        self.width = width;
        self.height = height;
        self
    }

    pub fn image_usage(mut self, usage: vk::ImageUsageFlags) -> Self {
        self.image_usage = usage;
        self
    }

    pub fn present_mode(mut self, mode: vk::PresentModeKHR) -> Self {
        self.present_mode = Some(mode);
        self
    }

    pub fn format(mut self, format: vk::Format, color_space: vk::ColorSpaceKHR) -> Self {
        self.format = Some(format);
        self.color_space = Some(color_space);
        self
    }

    pub fn old_swapchain(mut self, old: vk::SwapchainKHR) -> Self {
        self.old = old;
        self
    }

    pub fn build(self) -> Result<(Swapchain, Vec<vk::Image>)> {
        let surface = self.instance.surface();
        let surface_fn = self.instance.surface_fn();
        let pd = self.device.physical_device();

        let capabilities =
            unsafe { surface_fn.get_physical_device_surface_capabilities(pd, surface)? };
        let formats = unsafe { surface_fn.get_physical_device_surface_formats(pd, surface)? };
        let present_modes =
            unsafe { surface_fn.get_physical_device_surface_present_modes(pd, surface)? };

        let surface_format = if let (Some(fmt), Some(cs)) = (self.format, self.color_space) {
            formats
                .iter()
                .find(|f| f.format == fmt && f.color_space == cs)
                .copied()
                .unwrap_or(formats[0])
        } else {
            formats
                .iter()
                .find(|f| {
                    f.format == vk::Format::B8G8R8A8_SRGB
                        && f.color_space == vk::ColorSpaceKHR::SRGB_NONLINEAR
                })
                .copied()
                .unwrap_or(formats[0])
        };

        let present_mode = self.present_mode.unwrap_or_else(|| {
            if present_modes.contains(&vk::PresentModeKHR::MAILBOX) {
                vk::PresentModeKHR::MAILBOX
            } else {
                vk::PresentModeKHR::FIFO
            }
        });

        let extent = if capabilities.current_extent.width != u32::MAX {
            capabilities.current_extent
        } else {
            vk::Extent2D {
                width: self.width.clamp(
                    capabilities.min_image_extent.width,
                    capabilities.max_image_extent.width,
                ),
                height: self.height.clamp(
                    capabilities.min_image_extent.height,
                    capabilities.max_image_extent.height,
                ),
            }
        };

        let mut image_count = capabilities.min_image_count + 1;
        if capabilities.max_image_count > 0 && image_count > capabilities.max_image_count {
            image_count = capabilities.max_image_count;
        }

        let graphics_family = self.device.graphics_family();
        let present_family = self.device.present_family();
        let queue_family_indices = [graphics_family, present_family];

        let mut create_info = vk::SwapchainCreateInfoKHR::default()
            .surface(surface)
            .min_image_count(image_count)
            .image_format(surface_format.format)
            .image_color_space(surface_format.color_space)
            .image_extent(extent)
            .image_array_layers(1)
            .image_usage(self.image_usage)
            .pre_transform(capabilities.current_transform)
            .composite_alpha(vk::CompositeAlphaFlagsKHR::OPAQUE)
            .present_mode(present_mode)
            .clipped(true)
            .old_swapchain(self.old);

        if graphics_family != present_family {
            create_info = create_info
                .image_sharing_mode(vk::SharingMode::CONCURRENT)
                .queue_family_indices(&queue_family_indices);
        } else {
            create_info = create_info.image_sharing_mode(vk::SharingMode::EXCLUSIVE);
        }

        let swapchain_fn = khr::swapchain::Device::new(&*self.instance, &*self.device);
        let swapchain = unsafe { swapchain_fn.create_swapchain(&create_info, None)? };
        let images = unsafe { swapchain_fn.get_swapchain_images(swapchain)? };

        if self.old != vk::SwapchainKHR::null() {
            unsafe { swapchain_fn.destroy_swapchain(self.old, None) };
        }

        Ok((
            Swapchain {
                swapchain,
                swapchain_fn,
                format: surface_format.format,
                extent,
            },
            images,
        ))
    }
}
