use anyhow::Result;
use std::sync::Arc;

use ash::vk;
use ash_bootstrap::Device;

use super::{FrameContext, Pass};

pub struct ClearPass {
    device: Arc<Device>,
}

impl ClearPass {
    pub fn new(device: Arc<Device>) -> Result<Self> {
        Ok(Self { device })
    }
}

impl Pass for ClearPass {
    fn record(&self, ctx: &FrameContext) {
        ctx.cmd.transition_image(
            ctx.draw_image.image,
            vk::ImageLayout::UNDEFINED,
            vk::ImageLayout::GENERAL,
        );

        let mut clear_color = vk::ClearColorValue::default();
        clear_color.float32 = [0.0, 0.0, 1.0, 1.0];

        ctx.cmd.clear_color_image(
            ctx.draw_image.image,
            vk::ImageLayout::GENERAL,
            clear_color,
            vk::ImageSubresourceRange::default()
                .base_mip_level(0)
                .level_count(1)
                .base_array_layer(0)
                .layer_count(1)
                .aspect_mask(vk::ImageAspectFlags::COLOR),
        );
    }
}
