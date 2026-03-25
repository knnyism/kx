use anyhow::{Result, bail};
use std::{collections::BTreeMap, sync::Arc};

use ash::vk;
use ash_bootstrap::Device;

use super::ShaderMeta;

pub struct Pipeline {
    pub pipeline: vk::Pipeline,
    pub layout: vk::PipelineLayout,
    pub descriptor_set_layouts: Vec<vk::DescriptorSetLayout>,
}

impl Pipeline {
    pub fn builder() -> PipelineBuilder {
        PipelineBuilder::default()
    }

    pub fn destroy(&self, device: &Device) {
        unsafe {
            device.destroy_pipeline(self.pipeline, None);
            device.destroy_pipeline_layout(self.layout, None);
            for &descriptor_set_layout in &self.descriptor_set_layouts {
                device.destroy_descriptor_set_layout(descriptor_set_layout, None);
            }
        }
    }
}

pub struct PipelineBuilder {
    stages: Vec<(Vec<u8>, ShaderMeta)>,

    color_formats: Vec<vk::Format>,
    depth_format: Option<vk::Format>,
    depth_test: bool,
    depth_write: bool,
    depth_compare_op: vk::CompareOp,
    cull_mode: vk::CullModeFlags,
    front_face: vk::FrontFace,
    polygon_mode: vk::PolygonMode,
    topology: vk::PrimitiveTopology,
}

impl Default for PipelineBuilder {
    fn default() -> Self {
        Self {
            stages: Vec::default(),
            color_formats: Vec::default(),
            depth_format: Option::default(),
            depth_test: bool::default(),
            depth_write: bool::default(),
            depth_compare_op: vk::CompareOp::default(),
            cull_mode: vk::CullModeFlags::default(),
            front_face: vk::FrontFace::default(),
            polygon_mode: vk::PolygonMode::default(),
            topology: vk::PrimitiveTopology::default(),
        }
    }
}

impl PipelineBuilder {
    fn build_compute(
        &self,
        device: &Device,
        stage: &(Vec<u8>, ShaderMeta),
        layout: vk::PipelineLayout,
    ) -> Result<vk::Pipeline> {
        let module = self.create_shader_module(device, &stage.0)?;

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

        Ok(pipeline)
    }

    pub fn build(self, device: &Device) -> Result<Pipeline> {
        if self.stages.is_empty() {
            bail!("no stages specified");
        }

        let (descriptor_set_layouts, layout) = self.create_layout(device)?;

        let pipeline = self.build_compute(device, &self.stages[0], layout)?;

        Ok(Pipeline {
            pipeline,
            layout,
            descriptor_set_layouts: descriptor_set_layouts,
        })
    }

    pub fn shader(mut self, spv: &[u8], meta: &[u8]) -> Self {
        self.stages
            .push((spv.to_vec(), ShaderMeta::deserialize(meta)));
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

    pub fn depth_test(mut self, enable: bool) -> Self {
        self.depth_test = enable;
        self
    }

    pub fn depth_write(mut self, enable: bool) -> Self {
        self.depth_write = enable;
        self
    }

    pub fn depth_compare_op(mut self, op: vk::CompareOp) -> Self {
        self.depth_compare_op = op;
        self
    }

    pub fn cull_mode(mut self, mode: vk::CullModeFlags) -> Self {
        self.cull_mode = mode;
        self
    }

    pub fn front_face(mut self, face: vk::FrontFace) -> Self {
        self.front_face = face;
        self
    }

    pub fn polygon_mode(mut self, mode: vk::PolygonMode) -> Self {
        self.polygon_mode = mode;
        self
    }

    pub fn topology(mut self, topology: vk::PrimitiveTopology) -> Self {
        self.topology = topology;
        self
    }

    fn create_layout(
        &self,
        device: &Device,
    ) -> Result<(Vec<vk::DescriptorSetLayout>, vk::PipelineLayout)> {
        let mut merged: BTreeMap<(u32, u32), (vk::DescriptorType, u32, vk::ShaderStageFlags)> =
            BTreeMap::new();

        for (_, meta) in &self.stages {
            for b in &meta.bindings {
                let entry = merged.entry((b.set, b.binding)).or_insert((
                    b.descriptor_type,
                    b.count,
                    vk::ShaderStageFlags::empty(),
                ));
                entry.2 |= b.stage;
            }
        }

        let max_set = merged.keys().map(|(s, _)| *s).max().unwrap_or(0);
        let mut descriptor_set_layouts = Vec::new();

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

            let descriptor_set_layout = unsafe {
                let info = vk::DescriptorSetLayoutCreateInfo::default().bindings(&bindings);
                device.create_descriptor_set_layout(&info, None)?
            };
            descriptor_set_layouts.push(descriptor_set_layout);
        }

        let push_constant_ranges: Vec<vk::PushConstantRange> = self
            .stages
            .iter()
            .flat_map(|(_, meta)| &meta.push_constants)
            .map(|pc| {
                vk::PushConstantRange::default()
                    .stage_flags(pc.stage)
                    .offset(pc.offset)
                    .size(pc.size)
            })
            .collect();

        let layout = unsafe {
            let info = vk::PipelineLayoutCreateInfo::default()
                .set_layouts(&descriptor_set_layouts)
                .push_constant_ranges(&push_constant_ranges);
            device.create_pipeline_layout(&info, None)?
        };

        Ok((descriptor_set_layouts, layout))
    }

    fn create_shader_module(&self, device: &Device, spv: &[u8]) -> Result<vk::ShaderModule> {
        let code: Vec<u32> = spv
            .chunks_exact(4)
            .map(|c| u32::from_le_bytes(c.try_into().unwrap()))
            .collect();

        let info = vk::ShaderModuleCreateInfo::default().code(&code);
        Ok(unsafe { device.create_shader_module(&info, None)? })
    }
}
