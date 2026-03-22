pub mod command_buffer;
use command_buffer::*;

mod shader;
use shader::*;

mod image;
use image::*;

use anyhow::Result;
use std::sync::Arc;

use ash::vk;
use ash_bootstrap::{
    Device, DeviceBuilder, Instance, InstanceBuilder, PhysicalDeviceSelector, PreferredDeviceType,
    QueueType, Swapchain, SwapchainBuilder,
};
use gpu_allocator::vulkan::{Allocator, AllocatorCreateDesc};
use raw_window_handle::{DisplayHandle, WindowHandle};

const FRAMES_IN_FLIGHT: usize = 3;

struct QueueIndices {
    pub graphics: usize,
    pub present: usize,
}

struct Sync {
    pub in_flight: vk::Fence,
    pub image_available: vk::Semaphore,
}

pub struct Frame {
    pub command_buffer: CommandBuffer,
}

pub struct Graphics {
    instance: Arc<Instance>,
    device: Arc<Device>,
    swapchain: Swapchain,

    queue_indices: QueueIndices,
    graphics_queue: vk::Queue,
    present_queue: vk::Queue,

    swapchain_images: Vec<vk::Image>,
    swapchain_semaphores: Vec<vk::Semaphore>,

    command_pool: vk::CommandPool,
    command_buffers: Vec<CommandBuffer>,

    frame_syncs: Vec<Sync>,
    frame_index: usize,

    allocator: Allocator,

    draw_image: AllocatedImage,
    draw_extent: vk::Extent2D,
}

impl Drop for Graphics {
    fn drop(&mut self) {
        unsafe {
            let _ = self.device.device_wait_idle();

            self.device.destroy_command_pool(self.command_pool, None);

            for sync in &self.frame_syncs {
                self.device.destroy_fence(sync.in_flight, None);
                self.device.destroy_semaphore(sync.image_available, None);
            }

            for i in 0..self.swapchain_semaphores.len() {
                self.device
                    .destroy_semaphore(self.swapchain_semaphores[i], None);
            }

            destroy_image(&self.device, &mut self.allocator, &mut self.draw_image);

            let _ = self.swapchain.destroy_image_views();

            self.swapchain.destroy();
            self.device.destroy();
            self.instance.destroy();
        }
    }
}

impl Graphics {
    pub fn new(window_handle: WindowHandle, display_handle: DisplayHandle) -> Result<Self> {
        let instance = InstanceBuilder::new(Some((window_handle, display_handle)))
            .app_name("kx")
            .engine_name("kx-engine")
            .request_validation_layers(cfg!(debug_assertions))
            .require_api_version(vk::API_VERSION_1_3)
            .use_default_debug_messenger()
            .build()?;

        let features_12 = vk::PhysicalDeviceVulkan12Features::default().buffer_device_address(true);

        let features_13 = vk::PhysicalDeviceVulkan13Features::default()
            .synchronization2(true)
            .dynamic_rendering(true)
            .maintenance4(true);

        let physical_device = PhysicalDeviceSelector::new(instance.clone())
            .preferred_device_type(PreferredDeviceType::Discrete)
            .add_required_extension_feature(features_12)
            .add_required_extension_feature(features_13)
            .select()?;

        let device = Arc::new(DeviceBuilder::new(physical_device, instance.clone()).build()?);

        let (graphics_queue_index, graphics_queue) = device.get_queue(QueueType::Graphics)?;
        let (present_queue_index, present_queue) = device.get_queue(QueueType::Present)?;

        let queue_indices = QueueIndices {
            graphics: graphics_queue_index,
            present: present_queue_index,
        };

        let swapchain_builder = SwapchainBuilder::new(instance.clone(), device.clone());

        let swapchain = swapchain_builder
            .use_default_format_selection()
            .use_default_present_modes()
            .image_usage_flags(
                vk::ImageUsageFlags::COLOR_ATTACHMENT | vk::ImageUsageFlags::TRANSFER_DST,
            )
            .build()?;

        let swapchain_images = swapchain.get_images()?;
        let swapchain_semaphores = {
            let semaphore_info = vk::SemaphoreCreateInfo::default();
            (0..swapchain_images.len())
                .map(|_| unsafe { device.create_semaphore(&semaphore_info, None) })
                .collect::<Result<Vec<_>, _>>()?
        };

        let command_pool = unsafe {
            let create_info = vk::CommandPoolCreateInfo::default()
                .flags(vk::CommandPoolCreateFlags::RESET_COMMAND_BUFFER);
            device.create_command_pool(&create_info, None)?
        };

        let frame_syncs = (0..FRAMES_IN_FLIGHT)
            .map(|_| unsafe {
                Ok(Sync {
                    in_flight: device.create_fence(
                        &vk::FenceCreateInfo::default().flags(vk::FenceCreateFlags::SIGNALED),
                        None,
                    )?,
                    image_available: device
                        .create_semaphore(&vk::SemaphoreCreateInfo::default(), None)?,
                })
            })
            .collect::<Result<Vec<_>, vk::Result>>()?;

        let command_buffers = {
            (0..FRAMES_IN_FLIGHT)
                .map(|_| CommandBuffer::new(device.clone(), &command_pool))
                .collect::<Result<Vec<_>, _>>()?
        };

        let mut allocator = {
            let create_desc = AllocatorCreateDesc {
                instance: (*instance).as_ref().clone(),
                device: (*device).as_ref().clone(),
                physical_device: *device.physical_device().as_ref(), // awful!
                debug_settings: Default::default(),
                buffer_device_address: true,
                allocation_sizes: Default::default(),
            };

            Allocator::new(&create_desc)?
        };

        let draw_image = AllocatedImage::new(
            &device,
            &mut allocator,
            "render_target",
            vk::Extent3D::default()
                .width(swapchain.extent.width)
                .height(swapchain.extent.height)
                .depth(1),
            vk::Format::R16G16B16A16_SFLOAT,
            vk::ImageUsageFlags::TRANSFER_SRC
                | vk::ImageUsageFlags::TRANSFER_DST
                | vk::ImageUsageFlags::STORAGE
                | vk::ImageUsageFlags::COLOR_ATTACHMENT,
        )?;

        Ok(Self {
            instance,
            device,
            swapchain,

            queue_indices,
            graphics_queue,
            present_queue,

            swapchain_images,
            swapchain_semaphores,

            command_pool,
            command_buffers,

            frame_syncs,
            frame_index: 0,

            allocator,

            draw_image,
            draw_extent: vk::Extent2D::default(),
        })
    }

    pub fn draw(&mut self) -> Result<()> {
        let sync = &self.frame_syncs[self.frame_index];
        let command_buffer = &self.command_buffers[self.frame_index];

        unsafe {
            self.device
                .wait_for_fences(&[sync.in_flight], true, u64::MAX)?;

            let (image_index, _suboptimal) = match self.swapchain.acquire_next_image(
                *self.swapchain.as_ref(),
                u64::MAX,
                sync.image_available,
                vk::Fence::null(),
            ) {
                Ok(result) => result,
                Err(vk::Result::ERROR_OUT_OF_DATE_KHR) => return Ok(()),
                Err(e) => return Err(e.into()),
            };

            self.device.reset_fences(&[sync.in_flight])?;

            command_buffer.reset(vk::CommandBufferResetFlags::empty());

            command_buffer.begin(
                &vk::CommandBufferBeginInfo::default()
                    .flags(vk::CommandBufferUsageFlags::ONE_TIME_SUBMIT),
            );

            self.draw_extent.width = self.draw_image.extent.width;
            self.draw_extent.height = self.draw_image.extent.height;

            command_buffer.transition_image(
                self.draw_image.image,
                vk::ImageLayout::UNDEFINED,
                vk::ImageLayout::GENERAL,
            );

            let mut clear_color = vk::ClearColorValue::default();
            clear_color.float32 = [0.0, 0.0, 1.0, 1.0];

            command_buffer.clear_color_image(
                &self.draw_image,
                vk::ImageLayout::GENERAL,
                clear_color,
                vk::ImageSubresourceRange::default()
                    .base_mip_level(0)
                    .level_count(1)
                    .base_array_layer(0)
                    .layer_count(1)
                    .aspect_mask(vk::ImageAspectFlags::COLOR),
            );

            command_buffer.transition_image(
                self.draw_image.image,
                vk::ImageLayout::GENERAL,
                vk::ImageLayout::TRANSFER_SRC_OPTIMAL,
            );

            let swapchain_image = self.swapchain_images[image_index as usize];

            command_buffer.transition_image(
                swapchain_image,
                vk::ImageLayout::UNDEFINED,
                vk::ImageLayout::TRANSFER_DST_OPTIMAL,
            );

            command_buffer.copy_image_to_image(
                self.draw_image.image,
                swapchain_image,
                self.draw_extent,
                self.swapchain.extent,
            );

            command_buffer.transition_image(
                swapchain_image,
                vk::ImageLayout::TRANSFER_DST_OPTIMAL,
                vk::ImageLayout::PRESENT_SRC_KHR,
            );

            command_buffer.end();

            let cmd_info =
                vk::CommandBufferSubmitInfo::default().command_buffer(*command_buffer.as_ref());

            let wait_info = vk::SemaphoreSubmitInfo::default()
                .semaphore(sync.image_available)
                .stage_mask(vk::PipelineStageFlags2::COLOR_ATTACHMENT_OUTPUT)
                .value(1);

            let signal_info = vk::SemaphoreSubmitInfo::default()
                .semaphore(self.swapchain_semaphores[image_index as usize])
                .stage_mask(vk::PipelineStageFlags2::ALL_GRAPHICS)
                .value(1);

            let submit_info = vk::SubmitInfo2::default()
                .command_buffer_infos(std::slice::from_ref(&cmd_info))
                .wait_semaphore_infos(std::slice::from_ref(&wait_info))
                .signal_semaphore_infos(std::slice::from_ref(&signal_info));

            self.device
                .queue_submit2(self.graphics_queue, &[submit_info], sync.in_flight)?;

            let wait_semaphores = [self.swapchain_semaphores[image_index as usize]];
            let swapchains = [*self.swapchain.as_ref()];
            let image_indices = [image_index];

            let present_info = vk::PresentInfoKHR::default()
                .wait_semaphores(&wait_semaphores)
                .swapchains(&swapchains)
                .image_indices(&image_indices);

            match self
                .swapchain
                .queue_present(self.present_queue, &present_info)
            {
                Ok(is_suboptimal) if is_suboptimal => return Ok(()),
                Err(vk::Result::ERROR_OUT_OF_DATE_KHR) => return Ok(()),
                Err(e) => return Err(e.into()),
                _ => {}
            };
        };

        self.frame_index = (self.frame_index + 1) % FRAMES_IN_FLIGHT;
        Ok(())
    }

    pub fn create_shader_module(&self, spv: &[u8]) -> Result<vk::ShaderModule> {
        let code: Vec<u32> = spv
            .chunks_exact(4)
            .map(|chunk| u32::from_le_bytes(chunk.try_into().unwrap()))
            .collect();

        let create_info = vk::ShaderModuleCreateInfo::default().code(&code);

        Ok(unsafe { self.device.create_shader_module(&create_info, None)? })
    }
}
