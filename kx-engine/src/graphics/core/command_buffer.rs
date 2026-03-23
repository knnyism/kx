use anyhow::Result;
use std::sync::Arc;

use ash::vk;
use ash_bootstrap::Device;

pub struct CommandBuffer {
    device: Arc<Device>,
    command_buffer: vk::CommandBuffer,
}

impl CommandBuffer {
    pub fn new(device: Arc<Device>, command_pool: &vk::CommandPool) -> Result<Self> {
        let allocate_info = vk::CommandBufferAllocateInfo::default()
            .command_pool(*command_pool)
            .command_buffer_count(1);

        let command_buffer = unsafe { device.allocate_command_buffers(&allocate_info)? }[0];

        Ok(Self {
            device,
            command_buffer,
        })
    }

    pub fn reset(&self, flags: vk::CommandBufferResetFlags) {
        unsafe {
            self.device
                .reset_command_buffer(self.command_buffer, flags)
                .unwrap()
        };
    }

    pub fn begin(&self, begin_info: &vk::CommandBufferBeginInfo) {
        unsafe {
            self.device
                .begin_command_buffer(self.command_buffer, begin_info)
                .unwrap()
        };
    }

    pub fn end(&self) {
        unsafe { self.device.end_command_buffer(self.command_buffer).unwrap() };
    }

    pub fn bind_pipeline(&self, bind_point: vk::PipelineBindPoint, pipeline: vk::Pipeline) {
        unsafe {
            self.device
                .cmd_bind_pipeline(self.command_buffer, bind_point, pipeline);
        }
    }

    pub fn bind_descriptor_sets(
        &self,
        bind_point: vk::PipelineBindPoint,
        layout: vk::PipelineLayout,
        first_set: u32,
        descriptor_sets: &[vk::DescriptorSet],
    ) {
        unsafe {
            self.device.cmd_bind_descriptor_sets(
                self.command_buffer,
                bind_point,
                layout,
                first_set,
                descriptor_sets,
                &[],
            );
        }
    }

    pub fn dispatch(&self, group_count_x: u32, group_count_y: u32, group_count_z: u32) {
        unsafe {
            self.device.cmd_dispatch(
                self.command_buffer,
                group_count_x,
                group_count_y,
                group_count_z,
            );
        }
    }

    pub fn transition_image(
        &self,
        image: vk::Image,
        old_layout: vk::ImageLayout,
        new_layout: vk::ImageLayout,
    ) {
        let aspect_mask = if new_layout == vk::ImageLayout::DEPTH_ATTACHMENT_OPTIMAL
            || old_layout == vk::ImageLayout::DEPTH_ATTACHMENT_OPTIMAL
            || new_layout == vk::ImageLayout::DEPTH_STENCIL_ATTACHMENT_OPTIMAL
            || old_layout == vk::ImageLayout::DEPTH_STENCIL_ATTACHMENT_OPTIMAL
        {
            vk::ImageAspectFlags::DEPTH
        } else {
            vk::ImageAspectFlags::COLOR
        };

        let image_barrier = vk::ImageMemoryBarrier2::default()
            .src_stage_mask(vk::PipelineStageFlags2::ALL_COMMANDS)
            .src_access_mask(vk::AccessFlags2::MEMORY_WRITE)
            .dst_stage_mask(vk::PipelineStageFlags2::ALL_COMMANDS)
            .dst_access_mask(vk::AccessFlags2::MEMORY_WRITE | vk::AccessFlags2::MEMORY_READ)
            .old_layout(old_layout)
            .new_layout(new_layout)
            .src_queue_family_index(vk::QUEUE_FAMILY_IGNORED)
            .dst_queue_family_index(vk::QUEUE_FAMILY_IGNORED)
            .image(image)
            .subresource_range(vk::ImageSubresourceRange {
                aspect_mask,
                base_mip_level: 0,
                level_count: vk::REMAINING_MIP_LEVELS,
                base_array_layer: 0,
                layer_count: vk::REMAINING_ARRAY_LAYERS,
            });

        let dependency_info = vk::DependencyInfo::default()
            .image_memory_barriers(std::slice::from_ref(&image_barrier));

        unsafe {
            self.device
                .cmd_pipeline_barrier2(self.command_buffer, &dependency_info)
        };
    }

    pub fn copy_image_to_image(
        &self,
        src: vk::Image,
        dst: vk::Image,
        src_size: vk::Extent2D,
        dst_size: vk::Extent2D,
    ) {
        let src_subresource = vk::ImageSubresourceLayers {
            aspect_mask: vk::ImageAspectFlags::COLOR,
            mip_level: 0,
            base_array_layer: 0,
            layer_count: 1,
        };

        let blit_region = vk::ImageBlit2::default()
            .src_offsets([
                vk::Offset3D::default(),
                vk::Offset3D {
                    x: src_size.width as i32,
                    y: src_size.height as i32,
                    z: 1,
                },
            ])
            .dst_offsets([
                vk::Offset3D::default(),
                vk::Offset3D {
                    x: dst_size.width as i32,
                    y: dst_size.height as i32,
                    z: 1,
                },
            ])
            .src_subresource(src_subresource)
            .dst_subresource(src_subresource);

        let blit_info = vk::BlitImageInfo2::default()
            .src_image(src)
            .src_image_layout(vk::ImageLayout::TRANSFER_SRC_OPTIMAL)
            .dst_image(dst)
            .dst_image_layout(vk::ImageLayout::TRANSFER_DST_OPTIMAL)
            .filter(vk::Filter::LINEAR)
            .regions(std::slice::from_ref(&blit_region));

        unsafe {
            self.device.cmd_blit_image2(self.command_buffer, &blit_info);
        }
    }

    pub fn clear_color_image(
        &self,
        image: vk::Image,
        layout: vk::ImageLayout,
        value: vk::ClearColorValue,
        range: vk::ImageSubresourceRange,
    ) {
        unsafe {
            self.device
                .cmd_clear_color_image(self.command_buffer, image, layout, &value, &[range]);
        };
    }
}

impl AsRef<vk::CommandBuffer> for CommandBuffer {
    fn as_ref(&self) -> &vk::CommandBuffer {
        &self.command_buffer
    }
}
