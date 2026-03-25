use ash::vk;

struct BufferWrite {
    binding: u32,
    descriptor_type: vk::DescriptorType,
    info: vk::DescriptorBufferInfo,
}

struct ImageWrite {
    binding: u32,
    array_element: u32,
    descriptor_type: vk::DescriptorType,
    info: vk::DescriptorImageInfo,
}

#[derive(Default)]
pub struct DescriptorWriter {
    buffer_writes: Vec<BufferWrite>,
    image_writes: Vec<ImageWrite>,
}

impl DescriptorWriter {
    pub fn write_buffer(
        &mut self,
        binding: u32,
        buffer: vk::Buffer,
        size: u64,
        offset: u64,
        descriptor_type: vk::DescriptorType,
    ) -> &mut Self {
        self.buffer_writes.push(BufferWrite {
            binding,
            descriptor_type,
            info: vk::DescriptorBufferInfo::default()
                .buffer(buffer)
                .offset(offset)
                .range(size),
        });
        self
    }

    pub fn write_image(
        &mut self,
        binding: u32,
        image_view: vk::ImageView,
        sampler: vk::Sampler,
        layout: vk::ImageLayout,
        descriptor_type: vk::DescriptorType,
        array_element: u32,
    ) -> &mut Self {
        self.image_writes.push(ImageWrite {
            binding,
            array_element,
            descriptor_type,
            info: vk::DescriptorImageInfo::default()
                .sampler(sampler)
                .image_view(image_view)
                .image_layout(layout),
        });
        self
    }

    pub fn clear(&mut self) -> &mut Self {
        self.buffer_writes.clear();
        self.image_writes.clear();
        self
    }

    pub fn update_set(&mut self, device: &ash::Device, set: vk::DescriptorSet) {
        let mut writes: Vec<vk::WriteDescriptorSet> =
            Vec::with_capacity(self.buffer_writes.len() + self.image_writes.len());

        for bw in &self.buffer_writes {
            writes.push(
                vk::WriteDescriptorSet::default()
                    .dst_set(set)
                    .dst_binding(bw.binding)
                    .descriptor_type(bw.descriptor_type)
                    .buffer_info(std::slice::from_ref(&bw.info)),
            );
        }

        for iw in &self.image_writes {
            writes.push(
                vk::WriteDescriptorSet::default()
                    .dst_set(set)
                    .dst_binding(iw.binding)
                    .dst_array_element(iw.array_element)
                    .descriptor_type(iw.descriptor_type)
                    .image_info(std::slice::from_ref(&iw.info)),
            );
        }

        unsafe { device.update_descriptor_sets(&writes, &[]) };

        self.clear();
    }
}

#[derive(Clone)]
pub struct PoolSizeRatio {
    pub descriptor_type: vk::DescriptorType,
    pub ratio: f32,
}

pub struct DescriptorAllocator {
    ratios: Vec<PoolSizeRatio>,
    full_pools: Vec<vk::DescriptorPool>,
    ready_pools: Vec<vk::DescriptorPool>,
    sets_per_pool: u32,
}

impl DescriptorAllocator {
    pub fn new(device: &ash::Device, max_sets: u32, pool_ratios: Vec<PoolSizeRatio>) -> Self {
        let new_pool = Self::create_pool(device, max_sets, &pool_ratios);

        Self {
            ratios: pool_ratios,
            full_pools: Vec::new(),
            ready_pools: vec![new_pool],
            sets_per_pool: (max_sets as f32 * 1.5) as u32,
        }
    }

    pub fn allocate(
        &mut self,
        device: &ash::Device,
        layout: vk::DescriptorSetLayout,
    ) -> vk::DescriptorSet {
        let pool = self.get_pool(device);

        let layouts = [layout];
        let alloc_info = vk::DescriptorSetAllocateInfo::default()
            .descriptor_pool(pool)
            .set_layouts(&layouts);

        match unsafe { device.allocate_descriptor_sets(&alloc_info) } {
            Ok(sets) => {
                self.ready_pools.push(pool);
                sets[0]
            }
            Err(vk::Result::ERROR_OUT_OF_POOL_MEMORY | vk::Result::ERROR_FRAGMENTED_POOL) => {
                self.full_pools.push(pool);

                let new_pool = self.get_pool(device);
                self.ready_pools.push(new_pool);

                let alloc_info = vk::DescriptorSetAllocateInfo::default()
                    .descriptor_pool(new_pool)
                    .set_layouts(&layouts);

                unsafe { device.allocate_descriptor_sets(&alloc_info).unwrap()[0] }
            }
            Err(e) => panic!("descriptor set allocation failed: {e}"),
        }
    }

    pub fn clear_pools(&mut self, device: &ash::Device) {
        for &pool in &self.ready_pools {
            unsafe {
                device
                    .reset_descriptor_pool(pool, vk::DescriptorPoolResetFlags::empty())
                    .unwrap();
            }
        }

        for &pool in &self.full_pools {
            unsafe {
                device
                    .reset_descriptor_pool(pool, vk::DescriptorPoolResetFlags::empty())
                    .unwrap();
            }
            self.ready_pools.push(pool);
        }

        self.full_pools.clear();
    }

    pub fn destroy(&mut self, device: &ash::Device) {
        for &pool in &self.ready_pools {
            unsafe { device.destroy_descriptor_pool(pool, None) };
        }
        self.ready_pools.clear();

        for &pool in &self.full_pools {
            unsafe { device.destroy_descriptor_pool(pool, None) };
        }
        self.full_pools.clear();
    }

    fn get_pool(&mut self, device: &ash::Device) -> vk::DescriptorPool {
        if let Some(pool) = self.ready_pools.pop() {
            return pool;
        }

        let pool = Self::create_pool(device, self.sets_per_pool, &self.ratios);

        self.sets_per_pool = ((self.sets_per_pool as f32 * 1.5) as u32).min(4092);

        pool
    }

    fn create_pool(
        device: &ash::Device,
        set_count: u32,
        pool_ratios: &[PoolSizeRatio],
    ) -> vk::DescriptorPool {
        let pool_sizes: Vec<vk::DescriptorPoolSize> = pool_ratios
            .iter()
            .map(|r| {
                vk::DescriptorPoolSize::default()
                    .ty(r.descriptor_type)
                    .descriptor_count((r.ratio * set_count as f32) as u32)
            })
            .collect();

        let create_info = vk::DescriptorPoolCreateInfo::default()
            .max_sets(set_count)
            .pool_sizes(&pool_sizes);

        unsafe { device.create_descriptor_pool(&create_info, None).unwrap() }
    }
}
