use anyhow::{anyhow, Result};
use vulkanalia::{
    vk::{self, DeviceV1_0, HasBuilder, InstanceV1_0},
    Device, Instance,
};

use super::renderer::RendererData;

use std::ptr::copy_nonoverlapping;

#[derive(Default)]
pub struct Buffer {
    pub buffer: vk::Buffer,
    pub memory: vk::DeviceMemory,
}

impl Buffer {
    pub unsafe fn create(
        instance: &Instance,
        device: &Device,
        data: &RendererData,
        size: usize,
        usage: vk::BufferUsageFlags,
    ) -> Result<Self> {
        let info = vk::BufferCreateInfo::builder()
            .size(size as u64)
            .usage(usage);
        let buffer = device.create_buffer(&info, None)?;

        let memory_requirements = device.get_buffer_memory_requirements(buffer);

        let memory_info = vk::MemoryAllocateInfo::builder()
            .allocation_size(memory_requirements.size)
            .memory_type_index(get_memory_type_index(
                instance,
                data,
                vk::MemoryPropertyFlags::HOST_VISIBLE,
                memory_requirements,
            )?);

        let memory = device.allocate_memory(&memory_info, None)?;

        device.bind_buffer_memory(buffer, memory, 0)?;

        Ok(Self { buffer, memory })
    }

    pub unsafe fn fill<T>(&mut self, device: &Device, data: *const T) -> Result<()> {
        let memory = device.map_memory(
            self.memory,
            0,
            vk::WHOLE_SIZE as u64,
            vk::MemoryMapFlags::empty(),
        )?;

        copy_nonoverlapping(data, memory.cast(), 1);

        let memory_ranges = &[vk::MappedMemoryRange::builder()
            .memory(self.memory)
            .offset(0)
            .size(vk::WHOLE_SIZE as u64)];
        device.flush_mapped_memory_ranges(memory_ranges)?;

        device.unmap_memory(self.memory);

        Ok(())
    }

    pub unsafe fn destroy(&self, device: &Device) {
        device.destroy_buffer(self.buffer, None);
        device.free_memory(self.memory, None);
    }
}

unsafe fn get_memory_type_index(
    instance: &Instance,
    data: &RendererData,
    properties: vk::MemoryPropertyFlags,
    requirements: vk::MemoryRequirements,
) -> Result<u32> {
    let memory = instance.get_physical_device_memory_properties(data.physical_device);
    (0..memory.memory_type_count)
        .find(|i| {
            let suitable = (requirements.memory_type_bits & (1 << i)) != 0;
            let memory_type = memory.memory_types[*i as usize];
            suitable && memory_type.property_flags.contains(properties)
        })
        .ok_or_else(|| anyhow!("Failed to find suitable memory type."))
}
