pub mod core;
use core::*;

mod pass;
use pass::*;

use anyhow::Result;

use ash::{ext, khr, vk};
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
    instance: Instance,
    device: Device,
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
    triangle_pass: TrianglePass,
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

            for &semaphore in &self.swapchain_semaphores {
                self.device.destroy_semaphore(semaphore, None);
            }

            self.triangle_pass.destroy(&self.device);
            self.clear_pass.destroy(&self.device);

            for descriptor_allocator in &mut self.descriptor_allocators {
                descriptor_allocator.destroy(&self.device);
            }

            self.draw_image.destroy(&self.device, &mut self.allocator);

            self.swapchain.destroy();
            self.device.destroy();
            self.instance.destroy();
        }
    }
}

impl Graphics {
    pub fn new(window_handle: WindowHandle, display_handle: DisplayHandle) -> Result<Self> {
        let instance = InstanceBuilder::new()
            .app_name("kx")
            .engine_name("kx-engine")
            .api_version(vk::API_VERSION_1_3)
            .validation(cfg!(debug_assertions))
            .debug_messenger(cfg!(debug_assertions))
            .build(window_handle, display_handle)?;

        let features_12 = vk::PhysicalDeviceVulkan12Features::default().buffer_device_address(true);
        let features_13 = vk::PhysicalDeviceVulkan13Features::default()
            .synchronization2(true)
            .dynamic_rendering(true)
            .maintenance4(true);
        let local_read_features = vk::PhysicalDeviceDynamicRenderingLocalReadFeaturesKHR::default()
            .dynamic_rendering_local_read(true);
        let mesh_shader_features = vk::PhysicalDeviceMeshShaderFeaturesEXT::default()
            .task_shader(true)
            .mesh_shader(true);

        let physical_device = PhysicalDeviceSelector::new(&instance)
            .prefer_type(vk::PhysicalDeviceType::DISCRETE_GPU)
            .require_extension(khr::swapchain::NAME)
            .require_extension(ext::mesh_shader::NAME)
            .require_extension(khr::dynamic_rendering_local_read::NAME)
            .add_required_extension_feature(features_12)
            .add_required_extension_feature(features_13)
            .add_required_extension_feature(mesh_shader_features)
            .add_required_extension_feature(local_read_features)
            .select()?;

        let (device, graphics_queue, present_queue) =
            DeviceBuilder::new(&instance, physical_device).build()?;

        let (swapchain, swapchain_images) = SwapchainBuilder::new(&instance, &device)
            .image_usage(vk::ImageUsageFlags::COLOR_ATTACHMENT | vk::ImageUsageFlags::TRANSFER_DST)
            .size(1024, 768) // TODO: LOL!
            .build()?;

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
                .map(|_| CommandBuffer::new(device.clone(), command_pool))
                .collect::<Result<Vec<_>, _>>()?
        };

        let mut allocator = {
            let create_desc = AllocatorCreateDesc {
                instance: instance.clone(),
                device: (*device).clone(),
                physical_device: device.physical_device(),
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
        let triangle_pass = TrianglePass::new(&device, draw_image.format)?;

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
            triangle_pass,
        })
    }

    fn begin_frame(&mut self) -> Result<(u32, bool)> {
        let sync = &self.frame_syncs[self.frame_index];
        let command_buffer = &mut self.command_buffers[self.frame_index];

        unsafe {
            self.device
                .wait_for_fences(&[sync.in_flight], true, u64::MAX)?
        };
        let (image_index, _suboptimal) = match self.swapchain.acquire_next_image(
            u64::MAX,
            sync.image_available,
            vk::Fence::null(),
        ) {
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

        match self.swapchain.present(self.present_queue, &present_info) {
            Ok(true) | Err(vk::Result::ERROR_OUT_OF_DATE_KHR) => return Ok(()),
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
            cmd: &mut self.command_buffers[self.frame_index],
            rt: &self.draw_image,
            dalloc: &mut self.descriptor_allocators[self.frame_index],
        };

        self.clear_pass.record(&mut ctx);
        self.triangle_pass.record(&mut ctx);

        self.end_frame(image_index)?;

        self.frame_index = (self.frame_index + 1) % FRAMES_IN_FLIGHT;

        Ok(())
    }
}
