use anyhow::{anyhow, Result};
use vulkanalia::{
    vk::{self, InstanceV1_0},
};

use super::{images::Image, renderer::RendererData};

pub struct DepthBuffer {
    pub image: Image,
}

impl DepthBuffer {
    pub unsafe fn create(data: &RendererData) -> Result<Self> {
        Ok(Self {
            image: Image::create(
                data,
                (
                    data.swapchain.as_ref().unwrap().extent.width,
                    data.swapchain.as_ref().unwrap().extent.height,
                ),
                get_depth_format(data)?,
                vk::ImageTiling::OPTIMAL,
                vk::ImageUsageFlags::DEPTH_STENCIL_ATTACHMENT,
                vk::MemoryPropertyFlags::DEVICE_LOCAL,
                vk::ImageAspectFlags::DEPTH,
            )?,
        })
    }
}

unsafe fn get_supported_format(
    data: &RendererData,
    candidates: &[vk::Format],
    tiling: vk::ImageTiling,
    features: vk::FormatFeatureFlags,
) -> Result<vk::Format> {
    candidates
        .iter()
        .cloned()
        .find(|f| {
            let properties = data
                .instance
                .get_physical_device_format_properties(data.physical_device, *f);

            match tiling {
                vk::ImageTiling::LINEAR => properties.linear_tiling_features.contains(features),
                vk::ImageTiling::OPTIMAL => properties.optimal_tiling_features.contains(features),
                _ => false,
            }
        })
        .ok_or_else(|| anyhow!("Failed to find supported format!"))
}

pub unsafe fn get_depth_format(data: &RendererData) -> Result<vk::Format> {
    let candidates = &[
        vk::Format::D32_SFLOAT,
        vk::Format::D32_SFLOAT_S8_UINT,
        vk::Format::D24_UNORM_S8_UINT,
    ];

    get_supported_format(
        data,
        candidates,
        vk::ImageTiling::OPTIMAL,
        vk::FormatFeatureFlags::DEPTH_STENCIL_ATTACHMENT,
    )
}
