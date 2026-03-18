use anyhow::Result;
use gpu_allocator::vulkan::{Allocator, AllocatorCreateDesc};
use std::sync::Arc;

use ash::vk;
use ash_bootstrap::{
    Device, DeviceBuilder, Instance, InstanceBuilder, PhysicalDeviceSelector, PreferredDeviceType,
    QueueType, Swapchain, SwapchainBuilder,
};
use raw_window_handle::{DisplayHandle, WindowHandle};

pub struct QueueIndices {
    pub graphics: usize,
    pub present: usize,
}

pub struct Graphics {
    instance: Arc<Instance>,
    device: Arc<Device>,
    swapchain: Swapchain,

    queue_indices: QueueIndices,
    graphics_queue: vk::Queue,
    present_queue: vk::Queue,

    in_flight_fences: Vec<vk::Fence>,
    image_available_semaphores: Vec<vk::Semaphore>,
    render_finished_semaphores: Vec<vk::Semaphore>,

    command_pool: vk::CommandPool,
    command_buffers: Vec<vk::CommandBuffer>,

    current_frame: usize,
    frames_in_flight: usize,

    allocator: Option<Allocator>,
}

impl Drop for Graphics {
    fn drop(&mut self) {
        unsafe {
            let _ = self.device.device_wait_idle();

            drop(self.allocator.take());

            self.device.destroy_command_pool(self.command_pool, None);

            for i in 0..self.frames_in_flight {
                self.device.destroy_fence(self.in_flight_fences[i], None);
                self.device
                    .destroy_semaphore(self.image_available_semaphores[i], None);
            }

            for i in 0..self.render_finished_semaphores.len() {
                self.device
                    .destroy_semaphore(self.render_finished_semaphores[i], None);
            }

            self.swapchain.destroy();
            self.device.destroy();
            self.instance.destroy();
        }
    }
}

// device.create_fence(&vk::FenceCreateInfo::default().flags(FenceCreateFlags::SIGNALED), None)?

impl Graphics {
    pub fn new(window_handle: WindowHandle, display_handle: DisplayHandle) -> Result<Self> {
        let instance = InstanceBuilder::new(Some((window_handle, display_handle)))
            .app_name("kx")
            .engine_name("kx-engine")
            .request_validation_layers(cfg!(debug_assertions))
            .require_api_version(vk::API_VERSION_1_3)
            .use_default_debug_messenger()
            .build()?;

        let features_13 = vk::PhysicalDeviceVulkan13Features::default()
            .synchronization2(true)
            .dynamic_rendering(true)
            .maintenance4(true);

        let physical_device = PhysicalDeviceSelector::new(instance.clone())
            .preferred_device_type(PreferredDeviceType::Discrete)
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
            .build()?;

        const FRAMES_IN_FLIGHT: usize = 3;
        let frame_count = swapchain.get_images()?.len();

        let command_pool = unsafe {
            let create_info = vk::CommandPoolCreateInfo::default()
                .flags(vk::CommandPoolCreateFlags::RESET_COMMAND_BUFFER);
            device.create_command_pool(&create_info, None)?
        };

        let command_buffers = unsafe {
            let allocate_info = vk::CommandBufferAllocateInfo::default()
                .command_pool(command_pool)
                .command_buffer_count(FRAMES_IN_FLIGHT as u32);

            device.allocate_command_buffers(&allocate_info)?
        };

        let in_flight_fences = {
            let fence_info = vk::FenceCreateInfo::default().flags(vk::FenceCreateFlags::SIGNALED);
            (0..FRAMES_IN_FLIGHT)
                .map(|_| unsafe { device.create_fence(&fence_info, None) })
                .collect::<Result<Vec<_>, _>>()?
        };

        let image_available_semaphores = {
            let semaphore_info = vk::SemaphoreCreateInfo::default();
            (0..FRAMES_IN_FLIGHT)
                .map(|_| unsafe { device.create_semaphore(&semaphore_info, None) })
                .collect::<Result<Vec<_>, _>>()?
        };

        let render_finished_semaphores = {
            let semaphore_info = vk::SemaphoreCreateInfo::default();
            (0..frame_count)
                .map(|_| unsafe { device.create_semaphore(&semaphore_info, None) })
                .collect::<Result<Vec<_>, _>>()?
        };

        let allocator = Some({
            let create_desc = AllocatorCreateDesc {
                instance: (*instance).as_ref().clone(),
                device: (*device).as_ref().clone(),
                physical_device: *device.physical_device().as_ref(), // awful!
                debug_settings: Default::default(),
                buffer_device_address: false,
                allocation_sizes: Default::default(),
            };

            Allocator::new(&create_desc)?
        });

        Ok(Self {
            instance,
            device,
            swapchain,

            queue_indices,
            graphics_queue,
            present_queue,

            in_flight_fences,
            image_available_semaphores,
            render_finished_semaphores,

            command_pool,
            command_buffers,

            current_frame: 0,
            frames_in_flight: FRAMES_IN_FLIGHT,

            allocator,
        })
    }

    pub fn draw(&mut self) -> Result<()> {
        let current_fence = self.in_flight_fences[self.current_frame];
        unsafe {
            self.device
                .wait_for_fences(&[current_fence], true, u64::MAX)?;

            let (image_index, _suboptimal) = match self.swapchain.acquire_next_image(
                *self.swapchain.as_ref(),
                u64::MAX,
                self.image_available_semaphores[self.current_frame],
                vk::Fence::null(),
            ) {
                Ok(result) => result,
                Err(vk::Result::ERROR_OUT_OF_DATE_KHR) => return Ok(()),
                Err(e) => return Err(e.into()),
            };

            self.device.reset_fences(&[current_fence])?;

            let command_buffer = self.command_buffers[self.current_frame];

            self.device
                .reset_command_buffer(command_buffer, vk::CommandBufferResetFlags::empty())?;

            self.device.begin_command_buffer(
                command_buffer,
                &vk::CommandBufferBeginInfo::default()
                    .flags(vk::CommandBufferUsageFlags::ONE_TIME_SUBMIT),
            )?;

            self.device.end_command_buffer(command_buffer)?;

            let cmd_info = vk::CommandBufferSubmitInfo::default().command_buffer(command_buffer);

            let wait_info = vk::SemaphoreSubmitInfo::default()
                .semaphore(self.image_available_semaphores[self.current_frame])
                .stage_mask(vk::PipelineStageFlags2::COLOR_ATTACHMENT_OUTPUT)
                .value(1);

            let signal_info = vk::SemaphoreSubmitInfo::default()
                .semaphore(self.render_finished_semaphores[image_index as usize])
                .stage_mask(vk::PipelineStageFlags2::ALL_GRAPHICS)
                .value(1);

            let submit_info = vk::SubmitInfo2::default()
                .command_buffer_infos(std::slice::from_ref(&cmd_info))
                .wait_semaphore_infos(std::slice::from_ref(&wait_info))
                .signal_semaphore_infos(std::slice::from_ref(&signal_info));

            self.device
                .queue_submit2(self.graphics_queue, &[submit_info], current_fence)?;

            let wait_semaphores = [self.render_finished_semaphores[image_index as usize]];
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
                Ok(_) => {}
                Err(vk::Result::ERROR_OUT_OF_DATE_KHR) => return Ok(()),
                Err(e) => return Err(e.into()),
            };
        };

        self.current_frame = (self.current_frame + 1) % self.frames_in_flight;
        Ok(())
    }
}
