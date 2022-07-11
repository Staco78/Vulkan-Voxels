use anyhow::{anyhow, Result};
use vulkanalia::{
    vk::{self, DeviceV1_0, HasBuilder},
    Device,
};

use super::{
    memory::{AllocRequirements, AllocUsage, Allocator, Block},
    renderer::RendererData,
};

use std::{
    mem::size_of,
    ptr::copy_nonoverlapping,
    sync::{self, Arc},
};

#[derive(Debug)]
pub struct Buffer {
    device: sync::Weak<Device>,
    allocator: sync::Weak<Allocator>,
    pub buffer: vk::Buffer,
    pub alloc: Block,
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

        let alloc = data.allocator.alloc(AllocRequirements::new(
            memory_requirements,
            AllocUsage::Staging,
        ))?;

        data.device
            .bind_buffer_memory(buffer, alloc.memory, alloc.offset)?;

        Ok(Self {
            device: Arc::downgrade(&data.device),
            allocator: Arc::downgrade(&data.allocator),
            buffer,
            alloc,
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
        assert!(size <= self.alloc.size);

        let memory = device.map_memory(
            self.alloc.memory,
            self.alloc.offset + offset,
            size,
            vk::MemoryMapFlags::empty(),
        )?;

        self.current_map_ranges = Some((offset, size));

        Ok(memory.cast())
    }

    pub unsafe fn unmap(&mut self, device: &Device) -> Result<()> {
        let map = self
            .current_map_ranges
            .ok_or_else(|| anyhow!("Buffer not mapped"))?;
        let memory_ranges = &[vk::MappedMemoryRange::builder()
            .memory(self.alloc.memory)
            .offset(self.alloc.offset + map.0)
            .size(map.1)];
        device.flush_mapped_memory_ranges(memory_ranges)?;

        device.unmap_memory(self.alloc.memory);

        self.current_map_ranges = None;

        Ok(())
    }
}

impl Drop for Buffer {
    fn drop(&mut self) {
        let device = self.device.upgrade().unwrap();
        unsafe {
            device.destroy_buffer(self.buffer, None);
            self.allocator.upgrade().unwrap().free(self.alloc);
        }
    }
}
