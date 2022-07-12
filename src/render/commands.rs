use anyhow::Result;
use vulkanalia::{
    vk::{self, DeviceV1_0, HasBuilder},
    Device,
};

use std::sync::{self, Arc};

use super::renderer::RendererData;

pub struct CommandPool {
    device: sync::Weak<Device>,
    pub pool: vk::CommandPool,
}

impl CommandPool {
    pub unsafe fn create(data: &RendererData, queue_family: u32) -> Result<Self> {
        let info = vk::CommandPoolCreateInfo::builder()
            .queue_family_index(queue_family)
            .flags(
                vk::CommandPoolCreateFlags::RESET_COMMAND_BUFFER
                    | vk::CommandPoolCreateFlags::TRANSIENT,
            );

        let pool = data.device.create_command_pool(&info, None)?;

        Ok(Self {
            pool,
            device: Arc::downgrade(&data.device),
        })
    }

    pub unsafe fn allocate_command_buffers(
        &self,
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

    // pub unsafe fn reset(&mut self, device: &Device) -> Result<()> {
    //     device.reset_command_pool(self.pool, vk::CommandPoolResetFlags::empty())?;
    //     Ok(())
    // }
}

impl Drop for CommandPool {
    fn drop(&mut self) {
        unsafe {
            self.device
                .upgrade()
                .unwrap()
                .destroy_command_pool(self.pool, None);
        }
    }
}

#[derive(Debug, Clone, Copy)]
pub struct CommandBuffer {
    pub buffer: vk::CommandBuffer,
}

impl CommandBuffer {
    #[inline]
    pub unsafe fn begin(&mut self, device: &Device) -> Result<()> {
        let info = vk::CommandBufferBeginInfo::builder()
            .flags(vk::CommandBufferUsageFlags::ONE_TIME_SUBMIT);
        device.begin_command_buffer(self.buffer, &info)?;
        Ok(())
    }

    #[inline]
    pub unsafe fn end(&mut self, device: &Device) -> Result<()> {
        device.end_command_buffer(self.buffer)?;
        Ok(())
    }
}
