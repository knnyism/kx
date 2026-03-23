pub mod clear;
pub use clear::*;

use super::{CommandBuffer, Image};

pub struct FrameContext<'a> {
    pub cmd: &'a CommandBuffer,
    pub draw_image: &'a Image,
}

pub trait Pass {
    fn record(&self, ctx: &FrameContext);
}
