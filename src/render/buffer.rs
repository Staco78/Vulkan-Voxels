use anyhow::Result;
use vulkanalia::{
    vk::{self, DeviceV1_0, HasBuilder},
    Device, Instance,
};

use super::{memory::get_memory_type_index, renderer::RendererData};

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
