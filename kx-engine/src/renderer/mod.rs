use anyhow::Result;
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

struct Sync {
    image_available: vk::Semaphore,
    render_finished: vk::Semaphore,
    in_flight_fence: vk::Fence,
}

pub struct Graphics {
    instance: Arc<Instance>,
    device: Arc<Device>,
    swapchain: Swapchain,

    queue_indices: QueueIndices,
    graphics_queue: vk::Queue,
    present_queue: vk::Queue,

    syncs: Vec<Sync>,
}

impl Graphics {
    pub fn new(window_handle: WindowHandle, display_handle: DisplayHandle) -> Result<Self> {
        let instance = InstanceBuilder::new(Some((window_handle, display_handle)))
            .app_name("kx")
            .engine_name("kx-engine")
            .request_validation_layers(true)
            .require_api_version(vk::API_VERSION_1_2)
            .use_default_debug_messenger()
            .build()?;

        let physical_device = PhysicalDeviceSelector::new(instance.clone())
            .preferred_device_type(PreferredDeviceType::Discrete)
            .select()?;

        let device = Arc::new(DeviceBuilder::new(physical_device, instance.clone()).build()?);

        let (graphics_queue_index, graphics_queue) = device.get_queue(QueueType::Graphics)?;
        let (present_queue_index, present_queue) = device.get_queue(QueueType::Present)?;

        let queue_indices = QueueIndices {
            graphics: graphics_queue_index,
            present: present_queue_index,
        };

        let swapchain_builder = SwapchainBuilder::new(instance.clone(), device.clone());

        let swapchain = swapchain_builder.build()?;

        let mut syncs = vec![];
        for _ in 0..swapchain.get_images().iter().len() {
            syncs.push(create_sync(&device)?);
        }

        Ok(Self {
            instance,
            device,
            swapchain,
            queue_indices,
            graphics_queue,
            present_queue,
            syncs,
        })
    }

    fn destroy_sync(&self, sync: &Sync) {
        unsafe {
            self.device.destroy_semaphore(sync.image_available, None);
            self.device.destroy_semaphore(sync.render_finished, None);
            self.device.destroy_fence(sync.in_flight_fence, None);
        }
    }
}

fn create_sync(device: &Device) -> Result<Sync> {
    let semaphore_create_info = vk::SemaphoreCreateInfo::default();
    let fence_create_info = vk::FenceCreateInfo::default();

    Ok(unsafe {
        Sync {
            image_available: device.create_semaphore(&semaphore_create_info, None)?,
            render_finished: device.create_semaphore(&semaphore_create_info, None)?,
            in_flight_fence: device.create_fence(&fence_create_info, None)?,
        }
    })
}

impl Drop for Graphics {
    fn drop(&mut self) {
        for sync in &self.syncs {
            self.destroy_sync(sync);
        }

        self.swapchain.destroy();
        self.device.destroy();
        self.instance.destroy();
    }
}
