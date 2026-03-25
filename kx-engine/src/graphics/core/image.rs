use anyhow::Result;

use ash::vk;
use gpu_allocator::{
    MemoryLocation,
    vulkan::{Allocation, AllocationCreateDesc, AllocationScheme, Allocator},
};

pub struct Image {
    pub image: vk::Image,
    pub view: vk::ImageView,
    pub extent: vk::Extent3D,
    pub format: vk::Format,
    pub allocation: Option<Allocation>,
}

impl Image {
    pub fn new(
        device: &ash::Device,
        allocator: &mut Allocator,
        name: &str,
        extent: vk::Extent3D,
        format: vk::Format,
        usage: vk::ImageUsageFlags,
    ) -> Result<Image> {
        let aspect_flag = if format == vk::Format::D32_SFLOAT {
            vk::ImageAspectFlags::DEPTH
        } else {
            vk::ImageAspectFlags::COLOR
        };

        let image = unsafe {
            let create_info = vk::ImageCreateInfo::default()
                .image_type(vk::ImageType::TYPE_2D)
                .extent(extent)
                .format(format)
                .samples(vk::SampleCountFlags::TYPE_1)
                .tiling(vk::ImageTiling::OPTIMAL)
                .usage(usage)
                .mip_levels(1)
                .array_layers(1);

            device.create_image(&create_info, None)?
        };

        let allocation = {
            let desc = AllocationCreateDesc {
                name,
                requirements: unsafe { device.get_image_memory_requirements(image) },
                location: MemoryLocation::GpuOnly,
                linear: false,
                allocation_scheme: AllocationScheme::DedicatedImage(image),
            };

            allocator.allocate(&desc)?
        };

        unsafe {
            device
                .bind_image_memory(image, allocation.memory(), allocation.offset())
                .unwrap();
        }

        let view = unsafe {
            let create_info = vk::ImageViewCreateInfo::default()
                .view_type(vk::ImageViewType::TYPE_2D)
                .image(image)
                .format(format)
                .subresource_range(
                    vk::ImageSubresourceRange::default()
                        .base_mip_level(0)
                        .level_count(1)
                        .base_array_layer(0)
                        .layer_count(1)
                        .aspect_mask(aspect_flag),
                );

            device.create_image_view(&create_info, None)?
        };

        Ok(Image {
            image,
            view,
            extent,
            format,
            allocation: Some(allocation),
        })
    }

    pub fn destroy(&mut self, device: &ash::Device, allocator: &mut Allocator) {
        unsafe {
            device.destroy_image(self.image, None);
            device.destroy_image_view(self.view, None);
        }
        allocator.free(self.allocation.take().unwrap()).unwrap();
    }
}
