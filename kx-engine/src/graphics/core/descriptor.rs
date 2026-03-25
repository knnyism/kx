use anyhow::Result;
use std::sync::Arc;

use ash::vk;

struct BufferWrite {
    binding: u32,
    descriptor_type: vk::DescriptorType,
    info: vk::DescriptorBufferInfo,
}

struct ImageWrite {
    binding: u32,
    array_element: u32,
    descriptor_type: vk::DescriptorType,
    info: vk::DescriptorImageInfo,
}

#[derive(Default)]
pub struct DescriptorWriter {
    buffer_writes: Vec<BufferWrite>,
    image_writes: Vec<ImageWrite>,
}
