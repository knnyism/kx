pub mod clear;
pub use clear::*;

pub mod triangle;
pub use triangle::*;

use super::{CommandBuffer, DescriptorAllocator, Image};

pub struct FrameContext<'a> {
    pub device: &'a ash::Device,
    pub cmd: &'a mut CommandBuffer,
    pub rt: &'a Image,
    pub dalloc: &'a mut DescriptorAllocator,
}

pub trait Pass {
    fn record(&self, ctx: &mut FrameContext);
}
