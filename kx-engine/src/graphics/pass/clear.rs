use anyhow::Result;

use ash::vk;

use super::{FrameContext, Pass};
use crate::graphics::Pipeline;

pub struct ClearPass {
    pipeline: Pipeline,
}

impl ClearPass {
    pub fn new(device: &ash::Device) -> Result<Self> {
        let pipeline = Pipeline::builder()
            .shader(
                include_bytes!(concat!(env!("OUT_DIR"), "/clear.cs.spv")),
                include_bytes!(concat!(env!("OUT_DIR"), "/clear.cs.meta")),
            )
            .build(device)?;

        Ok(Self { pipeline })
    }

    pub fn destroy(&self, device: &ash::Device) {
        self.pipeline.destroy(device);
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
