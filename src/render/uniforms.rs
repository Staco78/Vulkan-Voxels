use std::{marker::PhantomData, mem::size_of};

use super::{buffer::Buffer, renderer::RendererData};
use anyhow::Result;
use vulkanalia::{
    vk::{self, DeviceV1_0, HasBuilder},
    Device, Instance,
};

#[derive(Default)]
pub struct Uniforms<T> {
    pub descriptor_set_layout: vk::DescriptorSetLayout,
    pub buffers: Vec<Buffer>,
    pub descriptor_pool: vk::DescriptorPool,
    pub descriptor_sets: Vec<vk::DescriptorSet>,
    _marker: PhantomData<T>,
}

impl<T> Uniforms<T> {
    pub unsafe fn create(
        instance: &Instance,
        device: &Device,
        data: &RendererData,
    ) -> Result<Self> {
        let descriptor_set_layout = {
            let ubo_binding = vk::DescriptorSetLayoutBinding::builder()
                .binding(0)
                .descriptor_type(vk::DescriptorType::UNIFORM_BUFFER)
                .descriptor_count(1)
                .stage_flags(vk::ShaderStageFlags::VERTEX);

            let bindings = &[ubo_binding];
            let info = vk::DescriptorSetLayoutCreateInfo::builder().bindings(bindings);

            device.create_descriptor_set_layout(&info, None)?
        };

        let mut buffers = Vec::with_capacity(data.swapchain.images.len());
        for _ in 0..data.swapchain.images.len() {
            buffers.push(Buffer::create(
                instance,
                device,
                data,
                size_of::<T>(),
                vk::BufferUsageFlags::UNIFORM_BUFFER,
            )?);
        }

        let descriptor_pool = {
            let ubo_size = vk::DescriptorPoolSize::builder()
                .type_(vk::DescriptorType::UNIFORM_BUFFER)
                .descriptor_count(data.swapchain.images.len() as u32);

            let pool_sizes = &[ubo_size];
            let info = vk::DescriptorPoolCreateInfo::builder()
                .pool_sizes(pool_sizes)
                .max_sets(data.swapchain.images.len() as u32);

            device.create_descriptor_pool(&info, None)?
        };

        let descriptor_sets = {
            let layouts = vec![descriptor_set_layout; data.swapchain.images.len()];
            let info = vk::DescriptorSetAllocateInfo::builder()
                .descriptor_pool(descriptor_pool)
                .set_layouts(&layouts);

            let sets = device.allocate_descriptor_sets(&info)?;

            for i in 0..data.swapchain.images.len() {
                let info = vk::DescriptorBufferInfo::builder()
                    .buffer(buffers[i].buffer)
                    .offset(0)
                    .range(size_of::<T>() as u64);

                let buffer_info = &[info];
                let ubo_write = vk::WriteDescriptorSet::builder()
                    .dst_set(sets[i])
                    .dst_binding(0)
                    .dst_array_element(0)
                    .descriptor_type(vk::DescriptorType::UNIFORM_BUFFER)
                    .buffer_info(buffer_info);

                device.update_descriptor_sets(&[ubo_write], &[] as &[vk::CopyDescriptorSet]);
            }

            sets
        };

        Ok(Self {
            descriptor_set_layout,
            buffers,
            descriptor_pool,
            descriptor_sets,
            _marker: PhantomData,
        })
    }

    pub unsafe fn update(&mut self, device: &Device, image_index: usize, data: &T) -> Result<()> {
        self.buffers[image_index].fill(device, data)?;
        Ok(())
    }

    pub unsafe fn destroy(&self, device: &Device) {
        self.buffers
            .iter()
            .for_each(|buffer| buffer.destroy(device));
        device.destroy_descriptor_set_layout(self.descriptor_set_layout, None);
        device.destroy_descriptor_pool(self.descriptor_pool, None);
    }
}
