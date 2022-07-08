
use std::sync::{self, Arc};

use anyhow::Result;
use vulkanalia::{
    vk::{self, DeviceV1_0, HasBuilder},
    Device,
};

use super::renderer::RendererData;


pub struct Framebuffers {
    device: sync::Weak<Device>,
    framebuffers: Vec<vk::Framebuffer>,
}

impl Framebuffers {
    pub unsafe fn create(data: &RendererData) -> Result<Self> {
        let framebuffers = data
            .swapchain
            .as_ref()
            .unwrap()
            .image_views
            .iter()
            .map(|i| {
                let attachments = &[*i, data.depth_buffer.as_ref().unwrap().image.view];
                let create_info = vk::FramebufferCreateInfo::builder()
                    .render_pass(data.pipeline.as_ref().unwrap().render_pass)
                    .attachments(attachments)
                    .width(data.swapchain.as_ref().unwrap().extent.width)
                    .height(data.swapchain.as_ref().unwrap().extent.height)
                    .layers(1);

                data.device.create_framebuffer(&create_info, None)
            })
            .collect::<Result<Vec<_>, _>>()?;

        Ok(Self {
            framebuffers,
            device: Arc::downgrade(&data.device),
        })
    }
}

impl Drop for Framebuffers {
    fn drop(&mut self) {
        let device = self.device.upgrade().unwrap();
        unsafe {
            for f in self.framebuffers.iter() {
                device.destroy_framebuffer(*f, None);
            }
        }
    }
}

impl std::ops::Index<usize> for Framebuffers {
    type Output = vk::Framebuffer;

    #[inline]
    fn index(&self, index: usize) -> &Self::Output {
        &self.framebuffers[index]
    }
}
