pub mod clear;
pub use clear::*;

use super::{CommandBuffer, DescriptorAllocator, Image};

pub struct FrameContext<'a> {
    pub device: &'a ash::Device,
    pub cmd: &'a CommandBuffer,
    pub draw_image: &'a Image,
    pub descriptor_allocator: &'a mut DescriptorAllocator,
}

pub trait Pass {
    fn record(&self, ctx: &mut FrameContext);
}
