use anyhow::Result;

use ash::vk;

use super::{FrameContext, Pass};
use crate::graphics::{GraphicsPipelineBuilder, Pipeline};

pub struct TrianglePass {
    pipeline: Pipeline,
}

impl TrianglePass {
    pub fn new(device: &ash::Device, color_format: vk::Format) -> Result<Self> {
        let pipeline = GraphicsPipelineBuilder::new()
            .add_stage(
                include_bytes!(concat!(env!("OUT_DIR"), "/triangle.ms.spv")),
                include_bytes!(concat!(env!("OUT_DIR"), "/triangle.ms.meta")),
            )
            .add_stage(
                include_bytes!(concat!(env!("OUT_DIR"), "/triangle.ps.spv")),
                include_bytes!(concat!(env!("OUT_DIR"), "/triangle.ps.meta")),
            )
            .color_format(color_format)
            .set_cull_mode(vk::CullModeFlags::NONE, vk::FrontFace::COUNTER_CLOCKWISE)
            .disable_depth_test()
            .build(device)?;

        Ok(Self { pipeline })
    }

    pub fn destroy(&self, device: &ash::Device) {
        self.pipeline.destroy(device);
    }
}

impl Pass for TrianglePass {
    fn record(&self, ctx: &mut FrameContext) {
        ctx.cmd.transition_image(
            ctx.rt.image,
            vk::ImageLayout::GENERAL,
            vk::ImageLayout::COLOR_ATTACHMENT_OPTIMAL,
        );

        let color_attachment = vk::RenderingAttachmentInfo::default()
            .image_view(ctx.rt.view)
            .image_layout(vk::ImageLayout::COLOR_ATTACHMENT_OPTIMAL)
            .load_op(vk::AttachmentLoadOp::LOAD)
            .store_op(vk::AttachmentStoreOp::STORE);

        let rendering_info = vk::RenderingInfo::default()
            .render_area(vk::Rect2D {
                offset: vk::Offset2D::default(),
                extent: vk::Extent2D {
                    width: ctx.rt.extent.width,
                    height: ctx.rt.extent.height,
                },
            })
            .layer_count(1)
            .color_attachments(std::slice::from_ref(&color_attachment));

        ctx.cmd.begin_rendering(&rendering_info);

        ctx.cmd
            .set_viewport(ctx.rt.extent.width as f32, ctx.rt.extent.height as f32);
        ctx.cmd
            .set_scissor(ctx.rt.extent.width, ctx.rt.extent.height);

        ctx.cmd.bind_pipeline(&self.pipeline);
        ctx.cmd.draw_mesh_tasks(1, 1, 1);

        ctx.cmd.end_rendering();
    }
}
