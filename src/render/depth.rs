use anyhow::{anyhow, Result};
use vulkanalia::{
    vk::{self, InstanceV1_0},
    Device, Instance,
};

use super::{images::Image, renderer::RendererData};

#[derive(Default)]
pub struct DepthBuffer {
    pub image: Image,
}

impl DepthBuffer {
    pub unsafe fn create(
        instance: &Instance,
        device: &Device,
        data: &RendererData,
    ) -> Result<Self> {
        Ok(Self {
            image: Image::create(
                instance,
                device,
                data,
                (data.swapchain.extent.width, data.swapchain.extent.height),
                get_depth_format(instance, data)?,
                vk::ImageTiling::OPTIMAL,
                vk::ImageUsageFlags::DEPTH_STENCIL_ATTACHMENT,
                vk::MemoryPropertyFlags::DEVICE_LOCAL,
                vk::ImageAspectFlags::DEPTH,
            )?,
        })
    }

    pub unsafe fn destroy(&self, device: &Device) {
        self.image.destroy(device);
    }
}

unsafe fn get_supported_format(
    instance: &Instance,
    data: &RendererData,
    candidates: &[vk::Format],
    tiling: vk::ImageTiling,
    features: vk::FormatFeatureFlags,
) -> Result<vk::Format> {
    candidates
        .iter()
        .cloned()
        .find(|f| {
            let properties =
                instance.get_physical_device_format_properties(data.physical_device, *f);

            match tiling {
                vk::ImageTiling::LINEAR => properties.linear_tiling_features.contains(features),
                vk::ImageTiling::OPTIMAL => properties.optimal_tiling_features.contains(features),
                _ => false,
            }
        })
        .ok_or_else(|| anyhow!("Failed to find supported format!"))
}

pub unsafe fn get_depth_format(instance: &Instance, data: &RendererData) -> Result<vk::Format> {
    let candidates = &[
        vk::Format::D32_SFLOAT,
        vk::Format::D32_SFLOAT_S8_UINT,
        vk::Format::D24_UNORM_S8_UINT,
    ];

    get_supported_format(
        instance,
        data,
        candidates,
        vk::ImageTiling::OPTIMAL,
        vk::FormatFeatureFlags::DEPTH_STENCIL_ATTACHMENT,
    )
}
