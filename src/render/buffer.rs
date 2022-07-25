use anyhow::Result;
use vulkanalia::{
    vk::{self, DeviceV1_0, HasBuilder},
    Device,
};

use super::{
    memory::{AllocRequirements, AllocUsage, Allocator, Block},
    renderer::RendererData,
};

use std::sync::{self, Arc};

#[derive(Debug)]
pub struct Buffer {
    device: sync::Weak<Device>,
    allocator: sync::Weak<Allocator>,
    pub buffer: vk::Buffer,
    pub alloc: Block,
    pub ptr: *mut u8, // null if not staging buffer
}

unsafe impl Send for Buffer {}
unsafe impl Sync for Buffer {}

impl Buffer {
    pub unsafe fn create(
        data: &RendererData,
        size: usize,
        buffer_usage: vk::BufferUsageFlags,
        memory_usage: AllocUsage,
    ) -> Result<Self> {
        let info = vk::BufferCreateInfo::builder()
            .size(size as u64)
            .usage(buffer_usage)
            .sharing_mode(vk::SharingMode::EXCLUSIVE);
        let buffer = data.device.create_buffer(&info, None)?;

        let memory_requirements = data.device.get_buffer_memory_requirements(buffer);

        let (alloc, ptr) = data
            .allocator
            .alloc(AllocRequirements::new(memory_requirements, memory_usage))?;

        data.device
            .bind_buffer_memory(buffer, alloc.memory, alloc.offset)?;

        Ok(Self {
            device: Arc::downgrade(&data.device),
            allocator: Arc::downgrade(&data.allocator),
            buffer,
            alloc,
            ptr,
        })
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
