use anyhow::Result;
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

    pub fn shader(mut self, spv: &[u8], meta: &[u8]) -> Self {
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
}

impl GraphicsPipelineBuilder {}
