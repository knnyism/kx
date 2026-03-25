pub mod clear;
pub use clear::*;

use super::{CommandBuffer, DescriptorAllocator, Image};

pub struct FrameContext<'a> {
    pub device: &'a ash::Device,
    pub cmd: &'a CommandBuffer,
    pub rt: &'a Image,
    pub dsa: &'a mut DescriptorAllocator,
}

pub trait Pass {
    fn record(&self, ctx: &mut FrameContext);
}
