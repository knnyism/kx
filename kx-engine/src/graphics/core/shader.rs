use ash::vk;

#[derive(Debug, Clone)]
pub struct BindingInfo {
    pub set: u32,
    pub binding: u32,
    pub descriptor_type: vk::DescriptorType,
    pub count: u32,
    pub stage: vk::ShaderStageFlags,
}

#[derive(Debug, Clone)]
pub struct PushConstantInfo {
    pub offset: u32,
    pub size: u32,
    pub stage: vk::ShaderStageFlags,
}

#[derive(Default, Debug, Clone)]
pub struct ShaderMeta {
    pub stage: vk::ShaderStageFlags,
    pub bindings: Vec<BindingInfo>,
    pub push_constants: Vec<PushConstantInfo>,
}

impl ShaderMeta {
    pub fn deserialize(data: &[u8]) -> Self {
        let mut cursor = 0;

        let read_u32 = |cursor: &mut usize| -> u32 {
            let val = u32::from_le_bytes(data[*cursor..*cursor + 4].try_into().unwrap());
            *cursor += 4;
            val
        };

        let stage = vk::ShaderStageFlags::from_raw(read_u32(&mut cursor));

        let bindings_count = read_u32(&mut cursor);
        let bindings = (0..bindings_count)
            .map(|_| BindingInfo {
                set: read_u32(&mut cursor),
                binding: read_u32(&mut cursor),
                descriptor_type: vk::DescriptorType::from_raw(read_u32(&mut cursor) as i32),
                count: read_u32(&mut cursor),
                stage,
            })
            .collect();

        let push_constants_count = read_u32(&mut cursor);
        let push_constants = (0..push_constants_count)
            .map(|_| PushConstantInfo {
                offset: read_u32(&mut cursor),
                size: read_u32(&mut cursor),
                stage,
            })
            .collect();

        Self {
            stage,
            bindings,
            push_constants,
        }
    }
}
