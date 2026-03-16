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

pub struct Graphics {
    pub instance: Arc<Instance>,
    pub device: Arc<Device>,
    pub swapchain: Swapchain,

    pub queue_indices: QueueIndices,
    pub graphics_queue: vk::Queue,
    pub present_queue: vk::Queue,
}

impl Graphics {
    pub fn new(window_handle: WindowHandle, display_handle: DisplayHandle) -> Result<Self> {
        let instance = InstanceBuilder::new(Some((window_handle, display_handle)))
            .app_name("Example Vulkan Application")
            .engine_name("Example Vulkan Engine")
            .request_validation_layers(true)
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

        Ok(Self {
            instance,
            device,
            swapchain,
            queue_indices,
            graphics_queue,
            present_queue,
        })
    }
}

impl Drop for Graphics {
    fn drop(&mut self) {
        self.swapchain.destroy();
        self.device.destroy();
        self.instance.destroy();
    }
}
