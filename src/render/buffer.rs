use anyhow::{anyhow, Result};
use vulkanalia::{
    vk::{self, DeviceV1_0, HasBuilder},
    Device,
};

use super::{memory::get_memory_type_index, renderer::RendererData};

use std::{
    mem::size_of,
    ptr::copy_nonoverlapping,
    sync::{self, Arc},
};

pub struct Buffer {
    device: sync::Weak<Device>,
    pub buffer: vk::Buffer,
    pub memory: vk::DeviceMemory,
    current_map_ranges: Option<(u64, u64)>,
}

impl Buffer {
    pub unsafe fn create(
        data: &RendererData,
        size: usize,
        usage: vk::BufferUsageFlags,
    ) -> Result<Self> {
        let info = vk::BufferCreateInfo::builder()
            .size(size as u64)
            .usage(usage);
        let buffer = data.device.create_buffer(&info, None)?;

        let memory_requirements = data.device.get_buffer_memory_requirements(buffer);

        let memory_info = vk::MemoryAllocateInfo::builder()
            .allocation_size(memory_requirements.size)
            .memory_type_index(get_memory_type_index(
                data,
                vk::MemoryPropertyFlags::HOST_VISIBLE,
                memory_requirements,
            )?);

        let memory = data.device.allocate_memory(&memory_info, None)?;

        data.device.bind_buffer_memory(buffer, memory, 0)?;

        Ok(Self {
            device: Arc::downgrade(&data.device),
            buffer,
            memory,
            current_map_ranges: None,
        })
    }

    pub unsafe fn fill<T>(&mut self, device: &Device, data: *const T, count: usize) -> Result<()> {
        let memory = self.map::<T>(device, 0, (size_of::<T>() * count) as u64)?;

        copy_nonoverlapping(data, memory.cast(), count);

        self.unmap(device)?;

        Ok(())
    }

    pub unsafe fn map<T>(&mut self, device: &Device, offset: u64, size: u64) -> Result<*mut T> {
        assert!(self.current_map_ranges.is_none());

        let memory = device.map_memory(self.memory, offset, size, vk::MemoryMapFlags::empty())?;

        self.current_map_ranges = Some((offset, size));

        Ok(memory.cast())
    }

    pub unsafe fn unmap(&mut self, device: &Device) -> Result<()> {
        let map = self
            .current_map_ranges
            .ok_or(anyhow!("Buffer not mapped"))?;
        let memory_ranges = &[vk::MappedMemoryRange::builder()
            .memory(self.memory)
            .offset(map.0)
            .size(map.1)];
        device.flush_mapped_memory_ranges(memory_ranges)?;

        device.unmap_memory(self.memory);

        self.current_map_ranges = None;

        Ok(())
    }
}

impl Drop for Buffer {
    fn drop(&mut self) {
        let device = self.device.upgrade().unwrap();
        unsafe {
            device.destroy_buffer(self.buffer, None);
            device.free_memory(self.memory, None);
        }
    }
}
