use anyhow::{Result, bail};
use std::collections::BTreeMap;

use ash::vk;

use super::ShaderMeta;

pub struct Pipeline {
    pub pipeline: vk::Pipeline,
    pub layout: vk::PipelineLayout,
    pub set_layouts: Vec<vk::DescriptorSetLayout>,
    pub bind_point: vk::PipelineBindPoint,
}

impl Pipeline {
    pub fn destroy(&self, device: &ash::Device) {
        unsafe {
            device.destroy_pipeline(self.pipeline, None);
            device.destroy_pipeline_layout(self.layout, None);
            for &descriptor_set_layout in &self.set_layouts {
                device.destroy_descriptor_set_layout(descriptor_set_layout, None);
            }
        }
    }
}

#[derive(Default)]
struct ShaderStage {
    spv: Vec<u8>,
    meta: ShaderMeta,
}

fn create_layouts(
    device: &ash::Device,
    stages: &[ShaderStage],
) -> Result<(Vec<vk::DescriptorSetLayout>, vk::PipelineLayout)> {
    let mut merged: BTreeMap<(u32, u32), (vk::DescriptorType, u32, vk::ShaderStageFlags)> =
        BTreeMap::new();

    for stage in stages {
        for b in &stage.meta.bindings {
            let entry = merged.entry((b.set, b.binding)).or_insert((
                b.descriptor_type,
                b.count,
                vk::ShaderStageFlags::empty(),
            ));
            entry.2 |= b.stage;
        }
    }

    let max_set = merged.keys().map(|(s, _)| *s).max().unwrap_or(0);
    let mut set_layouts = Vec::new();

    for set_index in 0..=max_set {
        let bindings: Vec<vk::DescriptorSetLayoutBinding> = merged
            .iter()
            .filter(|((s, _), _)| *s == set_index)
            .map(|((_, binding), (ty, count, flags))| {
                vk::DescriptorSetLayoutBinding::default()
                    .binding(*binding)
                    .descriptor_type(*ty)
                    .descriptor_count((*count).max(1))
                    .stage_flags(*flags)
            })
            .collect();

        let layout = unsafe {
            let info = vk::DescriptorSetLayoutCreateInfo::default().bindings(&bindings);
            device.create_descriptor_set_layout(&info, None)?
        };
        set_layouts.push(layout);
    }

    let push_constant_ranges: Vec<vk::PushConstantRange> = stages
        .iter()
        .flat_map(|stage| &stage.meta.push_constants)
        .map(|pc| {
            vk::PushConstantRange::default()
                .stage_flags(pc.stage)
                .offset(pc.offset)
                .size(pc.size)
        })
        .collect();

    let layout = unsafe {
        let info = vk::PipelineLayoutCreateInfo::default()
            .set_layouts(&set_layouts)
            .push_constant_ranges(&push_constant_ranges);
        device.create_pipeline_layout(&info, None)?
    };

    Ok((set_layouts, layout))
}

fn create_shader_module(device: &ash::Device, spv: &[u8]) -> Result<vk::ShaderModule> {
    let code: Vec<u32> = spv
        .chunks_exact(4)
        .map(|c| u32::from_le_bytes(c.try_into().unwrap()))
        .collect();

    let info = vk::ShaderModuleCreateInfo::default().code(&code);
    Ok(unsafe { device.create_shader_module(&info, None)? })
}

pub struct ComputePipelineBuilder {
    stage: ShaderStage,
}

impl ComputePipelineBuilder {
    pub fn new() -> Self {
        Self {
            stage: ShaderStage::default(),
        }
    }

    pub fn set_stage(mut self, spv: &[u8], meta: &[u8]) -> Self {
        self.stage = ShaderStage {
            spv: spv.to_vec(),
            meta: ShaderMeta::deserialize(meta),
        };

        self
    }

    pub fn build(self, device: &ash::Device) -> Result<Pipeline> {
        let (set_layouts, layout) = create_layouts(device, std::slice::from_ref(&self.stage))?;

        let module = create_shader_module(device, &self.stage.spv)?;

        let stage_info = vk::PipelineShaderStageCreateInfo::default()
            .stage(vk::ShaderStageFlags::COMPUTE)
            .module(module)
            .name(c"main");

        let info = vk::ComputePipelineCreateInfo::default()
            .stage(stage_info)
            .layout(layout);

        let pipeline = unsafe {
            let result = device
                .create_compute_pipelines(vk::PipelineCache::null(), &[info], None)
                .map_err(|(_, e)| e)?[0];

            device.destroy_shader_module(module, None);
            result
        };

        Ok(Pipeline {
            pipeline,
            layout,
            set_layouts,
            bind_point: vk::PipelineBindPoint::COMPUTE,
        })
    }
}

pub struct GraphicsPipelineBuilder {
    stages: Vec<ShaderStage>,

    topology: vk::PrimitiveTopology,
    polygon_mode: vk::PolygonMode,
    cull_mode: vk::CullModeFlags,
    front_face: vk::FrontFace,

    color_formats: Vec<vk::Format>,
    depth_format: Option<vk::Format>,

    depth_test: bool,
    depth_write: bool,
    depth_compare_op: vk::CompareOp,

    blending: BlendMode,
}

enum BlendMode {
    None,
    Alpha,
}

impl GraphicsPipelineBuilder {
    pub fn new() -> Self {
        Self {
            stages: Vec::default(),

            topology: vk::PrimitiveTopology::default(),
            polygon_mode: vk::PolygonMode::default(),
            cull_mode: vk::CullModeFlags::default(),
            front_face: vk::FrontFace::default(),

            color_formats: Vec::default(),
            depth_format: None,

            depth_test: bool::default(),
            depth_write: bool::default(),
            depth_compare_op: vk::CompareOp::default(),

            blending: BlendMode::None,
        }
    }

    pub fn add_stage(mut self, spv: &[u8], meta: &[u8]) -> Self {
        self.stages.push(ShaderStage {
            spv: spv.to_vec(),
            meta: ShaderMeta::deserialize(meta),
        });

        self
    }

    pub fn set_topology(mut self, topology: vk::PrimitiveTopology) -> Self {
        self.topology = topology;

        self
    }

    pub fn set_polygon_mode(mut self, mode: vk::PolygonMode) -> Self {
        self.polygon_mode = mode;

        self
    }

    pub fn set_cull_mode(
        mut self,
        cull_mode: vk::CullModeFlags,
        front_face: vk::FrontFace,
    ) -> Self {
        self.cull_mode = cull_mode;
        self.front_face = front_face;

        self
    }

    pub fn color_format(mut self, format: vk::Format) -> Self {
        self.color_formats.push(format);

        self
    }

    pub fn depth_format(mut self, format: vk::Format) -> Self {
        self.depth_format = Some(format);

        self
    }

    pub fn disable_depth_test(mut self) -> Self {
        self.depth_test = false;
        self.depth_write = false;
        self.depth_compare_op = vk::CompareOp::NEVER;

        self
    }
    pub fn enable_depth_test(mut self, write: bool, compare_op: vk::CompareOp) -> Self {
        self.depth_test = true;
        self.depth_write = write;
        self.depth_compare_op = compare_op;

        self
    }

    pub fn enable_blending_alpha(mut self) -> Self {
        self.blending = BlendMode::Alpha;

        self
    }

    pub fn build(self, device: &ash::Device) -> Result<Pipeline> {
        if self.stages.is_empty() {
            bail!("no stages specified");
        }

        let (set_layouts, layout) = create_layouts(device, &self.stages)?;

        let mut modules = Vec::new();
        let mut stage_infos = Vec::new();

        for stage in &self.stages {
            let module = create_shader_module(device, &stage.spv)?;
            modules.push(module);
            stage_infos.push(
                vk::PipelineShaderStageCreateInfo::default()
                    .stage(stage.meta.stage)
                    .module(module)
                    .name(c"main"),
            );
        }

        let has_vertex = self
            .stages
            .iter()
            .any(|stage| stage.meta.stage == vk::ShaderStageFlags::VERTEX);

        let vertex_input = vk::PipelineVertexInputStateCreateInfo::default();
        let input_assembly =
            vk::PipelineInputAssemblyStateCreateInfo::default().topology(self.topology);

        let rasterization = vk::PipelineRasterizationStateCreateInfo::default()
            .polygon_mode(self.polygon_mode)
            .cull_mode(self.cull_mode)
            .front_face(self.front_face)
            .line_width(1.0);

        let multisample = vk::PipelineMultisampleStateCreateInfo::default()
            .rasterization_samples(vk::SampleCountFlags::TYPE_1);

        let depth_stencil = vk::PipelineDepthStencilStateCreateInfo::default()
            .depth_test_enable(self.depth_test)
            .depth_write_enable(self.depth_write)
            .depth_compare_op(self.depth_compare_op);

        let color_blend_attachments: Vec<vk::PipelineColorBlendAttachmentState> = self
            .color_formats
            .iter()
            .map(|_| {
                let mut att = vk::PipelineColorBlendAttachmentState::default()
                    .color_write_mask(vk::ColorComponentFlags::RGBA);

                if matches!(self.blending, BlendMode::Alpha) {
                    att = att
                        .blend_enable(true)
                        .src_color_blend_factor(vk::BlendFactor::SRC_ALPHA)
                        .dst_color_blend_factor(vk::BlendFactor::ONE_MINUS_SRC_ALPHA)
                        .color_blend_op(vk::BlendOp::ADD)
                        .src_alpha_blend_factor(vk::BlendFactor::ONE)
                        .dst_alpha_blend_factor(vk::BlendFactor::ZERO)
                        .alpha_blend_op(vk::BlendOp::ADD);
                }

                att
            })
            .collect();

        let color_blend =
            vk::PipelineColorBlendStateCreateInfo::default().attachments(&color_blend_attachments);

        let dynamic_states = [vk::DynamicState::VIEWPORT, vk::DynamicState::SCISSOR];
        let dynamic_state =
            vk::PipelineDynamicStateCreateInfo::default().dynamic_states(&dynamic_states);

        let viewport_state = vk::PipelineViewportStateCreateInfo::default()
            .viewport_count(1)
            .scissor_count(1);

        let mut rendering_info = vk::PipelineRenderingCreateInfo::default()
            .color_attachment_formats(&self.color_formats);

        if let Some(fmt) = self.depth_format {
            rendering_info = rendering_info.depth_attachment_format(fmt);
        }

        let mut info = vk::GraphicsPipelineCreateInfo::default()
            .stages(&stage_infos)
            .rasterization_state(&rasterization)
            .multisample_state(&multisample)
            .depth_stencil_state(&depth_stencil)
            .color_blend_state(&color_blend)
            .dynamic_state(&dynamic_state)
            .viewport_state(&viewport_state)
            .layout(layout)
            .push_next(&mut rendering_info);

        if has_vertex {
            info = info
                .vertex_input_state(&vertex_input)
                .input_assembly_state(&input_assembly);
        }

        let pipeline = unsafe {
            let result = device
                .create_graphics_pipelines(vk::PipelineCache::null(), &[info], None)
                .map_err(|(_, e)| e)?[0];

            for module in modules {
                device.destroy_shader_module(module, None);
            }

            result
        };

        Ok(Pipeline {
            pipeline,
            layout,
            set_layouts,
            bind_point: vk::PipelineBindPoint::GRAPHICS,
        })
    }
}
