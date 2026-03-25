pub mod core;
use core::*;

mod pass;
use pass::*;

use anyhow::Result;
use std::{ops::Deref, sync::Arc};

use ash::vk;
use ash_bootstrap::{
    DeviceBuilder, InstanceBuilder, PhysicalDeviceSelector, PreferredDeviceType, QueueType,
    Swapchain, SwapchainBuilder,
};
use gpu_allocator::vulkan::{Allocator, AllocatorCreateDesc};
use raw_window_handle::{DisplayHandle, WindowHandle};

const FRAMES_IN_FLIGHT: usize = 3;

struct Sync {
    pub in_flight: vk::Fence,
    pub image_available: vk::Semaphore,
}

pub struct Frame {
    pub command_buffer: CommandBuffer,
}

pub struct Graphics {
    instance: Arc<ash_bootstrap::Instance>,
    device: Arc<ash_bootstrap::Device>,

    swapchain: Swapchain,
    allocator: Allocator,

    graphics_queue: vk::Queue,
    present_queue: vk::Queue,

    swapchain_images: Vec<vk::Image>,
    swapchain_semaphores: Vec<vk::Semaphore>,

    command_pool: vk::CommandPool,
    command_buffers: Vec<CommandBuffer>,

    descriptor_allocators: Vec<DescriptorAllocator>,

    frame_syncs: Vec<Sync>,
    frame_index: usize,

    draw_image: Image,

    clear_pass: ClearPass,
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

            self.clear_pass.destroy(&self.device);

            for descriptor_allocator in &mut self.descriptor_allocators {
                descriptor_allocator.destroy(&self.device);
            }

            self.draw_image.destroy(&self.device, &mut self.allocator);

            let _ = self.swapchain.destroy_image_views();

            self.swapchain.destroy();
            self.device.destroy();
            self.instance.destroy();
        }
    }
}

impl Graphics {
    pub fn new(window_handle: WindowHandle, display_handle: DisplayHandle) -> Result<Self> {
        let features_12 = vk::PhysicalDeviceVulkan12Features::default().buffer_device_address(true);

        let features_13 = vk::PhysicalDeviceVulkan13Features::default()
            .synchronization2(true)
            .dynamic_rendering(true)
            .maintenance4(true);

        let instance = InstanceBuilder::new(Some((window_handle, display_handle)))
            .app_name("kx")
            .engine_name("kx-engine")
            .request_validation_layers(cfg!(debug_assertions))
            .require_api_version(vk::API_VERSION_1_3)
            .use_default_debug_messenger()
            .build()?;

        let physical_device = PhysicalDeviceSelector::new(instance.clone())
            .preferred_device_type(PreferredDeviceType::Discrete)
            .add_required_extension_feature(features_12)
            .add_required_extension_feature(features_13)
            .select()?;

        let device = Arc::new(DeviceBuilder::new(physical_device, instance.clone()).build()?);

        let (_, graphics_queue) = device.get_queue(QueueType::Graphics)?;
        let (_, present_queue) = device.get_queue(QueueType::Present)?;

        let swapchain_builder = SwapchainBuilder::new(instance.clone(), device.clone());

        let swapchain = swapchain_builder
            .use_default_format_selection()
            .use_default_present_modes()
            .image_usage_flags(
                vk::ImageUsageFlags::COLOR_ATTACHMENT | vk::ImageUsageFlags::TRANSFER_DST,
            )
            .desired_size(vk::Extent2D::default().width(1024).height(768)) // TODO: LOL!
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

        let ash_device = device.deref().as_ref().clone();
        let ash_instance = instance.deref().as_ref().clone();
        let vk_physical_device = device.physical_device().as_ref().clone();

        let command_buffers = {
            (0..FRAMES_IN_FLIGHT)
                .map(|_| CommandBuffer::new(ash_device.clone(), &command_pool))
                .collect::<Result<Vec<_>, _>>()?
        };

        let mut allocator = {
            let create_desc = AllocatorCreateDesc {
                instance: ash_instance.clone(),
                device: ash_device.clone(),
                physical_device: vk_physical_device.clone(), // awful!
                debug_settings: Default::default(),
                buffer_device_address: true,
                allocation_sizes: Default::default(),
            };

            Allocator::new(&create_desc)?
        };

        let ratios = vec![
            PoolSizeRatio {
                descriptor_type: vk::DescriptorType::STORAGE_IMAGE,
                ratio: 1.0,
            },
            PoolSizeRatio {
                descriptor_type: vk::DescriptorType::UNIFORM_BUFFER,
                ratio: 1.0,
            },
            PoolSizeRatio {
                descriptor_type: vk::DescriptorType::COMBINED_IMAGE_SAMPLER,
                ratio: 1.0,
            },
        ];

        let descriptor_allocators = (0..FRAMES_IN_FLIGHT)
            .map(|_| DescriptorAllocator::new(&device, 10, ratios.clone()))
            .collect::<Vec<_>>();

        let draw_image = Image::new(
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

        let clear_pass = ClearPass::new(&device)?;

        Ok(Self {
            instance,
            device,
            swapchain,
            allocator,

            graphics_queue,
            present_queue,

            swapchain_images,
            swapchain_semaphores,

            command_pool,
            command_buffers,

            descriptor_allocators,

            frame_syncs,
            frame_index: 0,

            draw_image,

            clear_pass,
        })
    }

    fn begin_frame(&mut self) -> Result<(u32, bool)> {
        let sync = &self.frame_syncs[self.frame_index];
        let command_buffer = &self.command_buffers[self.frame_index];

        unsafe {
            self.device
                .wait_for_fences(&[sync.in_flight], true, u64::MAX)?
        };
        let (image_index, _suboptimal) = match unsafe {
            self.swapchain.acquire_next_image(
                *self.swapchain.as_ref(),
                u64::MAX,
                sync.image_available,
                vk::Fence::null(),
            )
        } {
            Ok(result) => result,
            Err(vk::Result::ERROR_OUT_OF_DATE_KHR) => return Ok((0, true)),
            Err(e) => return Err(e.into()),
        };
        unsafe { self.device.reset_fences(&[sync.in_flight])? };
        command_buffer.reset(vk::CommandBufferResetFlags::empty());

        command_buffer.begin(
            &vk::CommandBufferBeginInfo::default()
                .flags(vk::CommandBufferUsageFlags::ONE_TIME_SUBMIT),
        );

        self.descriptor_allocators[self.frame_index].clear_pools(&self.device);

        Ok((image_index, false))
    }

    fn end_frame(&self, image_index: u32) -> Result<()> {
        let sync = &self.frame_syncs[self.frame_index];
        let command_buffer = &self.command_buffers[self.frame_index];
        let swapchain_image = self.swapchain_images[image_index as usize];

        command_buffer.transition_image(
            self.draw_image.image,
            vk::ImageLayout::UNDEFINED,
            vk::ImageLayout::TRANSFER_SRC_OPTIMAL,
        );

        command_buffer.transition_image(
            swapchain_image,
            vk::ImageLayout::UNDEFINED,
            vk::ImageLayout::TRANSFER_DST_OPTIMAL,
        );

        command_buffer.copy_image_to_image(
            self.draw_image.image,
            swapchain_image,
            vk::Extent2D::default()
                .width(self.draw_image.extent.width)
                .height(self.draw_image.extent.height),
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

        unsafe {
            self.device
                .queue_submit2(self.graphics_queue, &[submit_info], sync.in_flight)?
        };

        let wait_semaphores = [self.swapchain_semaphores[image_index as usize]];
        let swapchains = [*self.swapchain.as_ref()];
        let image_indices = [image_index];

        let present_info = vk::PresentInfoKHR::default()
            .wait_semaphores(&wait_semaphores)
            .swapchains(&swapchains)
            .image_indices(&image_indices);

        match unsafe {
            self.swapchain
                .queue_present(self.present_queue, &present_info)
        } {
            Ok(is_suboptimal) if is_suboptimal => return Ok(()),
            Err(vk::Result::ERROR_OUT_OF_DATE_KHR) => return Ok(()),
            Err(e) => return Err(e.into()),
            _ => {}
        };

        Ok(())
    }

    pub fn draw(&mut self) -> Result<()> {
        let (image_index, out_of_date) = self.begin_frame()?;
        if out_of_date {
            return Ok(());
        }

        let mut ctx = FrameContext {
            device: &self.device,
            cmd: &self.command_buffers[self.frame_index],
            rt: &self.draw_image,
            dsa: &mut self.descriptor_allocators[self.frame_index],
        };

        self.clear_pass.record(&mut ctx);

        self.end_frame(image_index)?;

        self.frame_index = (self.frame_index + 1) % FRAMES_IN_FLIGHT;

        Ok(())
    }
}
