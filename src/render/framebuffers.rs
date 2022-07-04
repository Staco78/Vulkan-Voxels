use anyhow::Result;
use vulkanalia::{
    vk::{self, DeviceV1_0, HasBuilder},
    Device,
};

use super::renderer::RendererData;

#[derive(Default)]
pub struct Framebuffers {
    framebuffers: Vec<vk::Framebuffer>,
}

impl Framebuffers {
    pub unsafe fn create(device: &Device, data: &RendererData) -> Result<Self> {
        let framebuffers = data
            .swapchain
            .image_views
            .iter()
            .map(|i| {
                let attachments = &[*i];
                let create_info = vk::FramebufferCreateInfo::builder()
                    .render_pass(data.pipeline.render_pass)
                    .attachments(attachments)
                    .width(data.swapchain.extent.width)
                    .height(data.swapchain.extent.height)
                    .layers(1);

                device.create_framebuffer(&create_info, None)
            })
            .collect::<Result<Vec<_>, _>>()?;

        Ok(Self { framebuffers })
    }

    pub unsafe fn destroy(&self, device: &Device) {
        for f in self.framebuffers.iter() {
            device.destroy_framebuffer(*f, None);
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
