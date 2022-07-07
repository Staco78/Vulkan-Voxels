use anyhow::Result;
use vulkanalia::{
    vk::{self, DeviceV1_0, HasBuilder},
    Device, Instance,
};

use super::{queue, renderer::RendererData};

#[derive(Default)]
pub struct CommandPool {
    pub pool: vk::CommandPool,
}

impl CommandPool {
    pub unsafe fn create(
        instance: &Instance,
        device: &Device,
        data: &RendererData,
    ) -> Result<Self> {
        let indices = queue::QueueFamilyIndices::get(instance, data, data.physical_device)?;

        let info = vk::CommandPoolCreateInfo::builder()
            .queue_family_index(indices.graphics)
            .flags(vk::CommandPoolCreateFlags::empty());

        let pool = device.create_command_pool(&info, None)?;

        Ok(Self { pool })
    }

    pub unsafe fn allocate_command_buffers(
        &mut self,
        device: &Device,
        count: u32,
    ) -> Result<Vec<CommandBuffer>> {
        let info = vk::CommandBufferAllocateInfo::builder()
            .command_pool(self.pool)
            .level(vk::CommandBufferLevel::PRIMARY)
            .command_buffer_count(count);

        let buffers = device.allocate_command_buffers(&info)?;

        let buffers = buffers
            .iter()
            .map(|b| CommandBuffer { buffer: *b })
            .collect();

        Ok(buffers)
    }

    pub unsafe fn reset(&mut self, device: &Device) -> Result<()> {
        device.reset_command_pool(self.pool, vk::CommandPoolResetFlags::empty())?;

        Ok(())
    }

    pub unsafe fn destroy(&self, device: &Device) {
        device.destroy_command_pool(self.pool, None);
    }
}

#[derive(Default)]
pub struct CommandBuffer {
    pub buffer: vk::CommandBuffer,
}

impl CommandBuffer {
    #[inline]
    pub unsafe fn begin(&mut self, device: &Device) -> Result<()> {
        let info = vk::CommandBufferBeginInfo::builder();
        device.begin_command_buffer(self.buffer, &info)?;
        Ok(())
    }

    #[inline]
    pub unsafe fn end(&mut self, device: &Device) -> Result<()> {
        device.end_command_buffer(self.buffer)?;
        Ok(())
    }
}
