use anyhow::Result;

use ash::vk;

use super::{FrameContext, Pass};
use crate::graphics::{DescriptorWriter, Pipeline};

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
    fn record(&self, ctx: &mut FrameContext) {
        ctx.cmd.transition_image(
            ctx.draw_image.image,
            vk::ImageLayout::UNDEFINED,
            vk::ImageLayout::GENERAL,
        );

        let set = ctx
            .descriptor_allocator
            .allocate(ctx.device, self.pipeline.descriptor_set_layouts[0]);

        DescriptorWriter::default()
            .write_image(
                0,
                ctx.draw_image.view,
                vk::Sampler::null(),
                vk::ImageLayout::GENERAL,
                vk::DescriptorType::STORAGE_IMAGE,
                0,
            )
            .update_set(ctx.device, set);

        ctx.cmd
            .bind_pipeline(vk::PipelineBindPoint::COMPUTE, self.pipeline.pipeline);

        ctx.cmd.bind_descriptor_sets(
            vk::PipelineBindPoint::COMPUTE,
            self.pipeline.layout,
            0,
            &[set],
        );

        let width = ctx.draw_image.extent.width;
        let height = ctx.draw_image.extent.height;

        ctx.cmd.dispatch((width + 15) / 16, (height + 15) / 16, 1);
    }
}
