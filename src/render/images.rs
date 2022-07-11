use std::sync::{self, Arc};

use super::{
    memory::{AllocRequirements, AllocUsage, Allocator, Block},
    renderer::RendererData,
};
use anyhow::Result;
use vulkanalia::{
    vk::{self, DeviceV1_0, HasBuilder},
    Device,
};

pub unsafe fn create_image_view(
    device: &Device,
    image: vk::Image,
    format: vk::Format,
    aspects: vk::ImageAspectFlags,
    mip_levels: u32,
) -> Result<vk::ImageView> {
    let components = vk::ComponentMapping::builder()
        .r(vk::ComponentSwizzle::IDENTITY)
        .g(vk::ComponentSwizzle::IDENTITY)
        .b(vk::ComponentSwizzle::IDENTITY)
        .a(vk::ComponentSwizzle::IDENTITY);

    let subresource_range = vk::ImageSubresourceRange::builder()
        .aspect_mask(aspects)
        .base_mip_level(0)
        .level_count(mip_levels)
        .base_array_layer(0)
        .layer_count(1);

    let info = vk::ImageViewCreateInfo::builder()
        .image(image)
        .view_type(vk::ImageViewType::_2D)
        .format(format)
        .subresource_range(subresource_range)
        .components(components);

    Ok(device.create_image_view(&info, None)?)
}

pub struct Image {
    device: sync::Weak<Device>,
    allocator: sync::Weak<Allocator>,
    pub image: vk::Image,
    pub alloc: Block,
    pub view: vk::ImageView,
}

impl Image {
    pub unsafe fn create(
        data: &RendererData,
        size: (u32, u32),
        format: vk::Format,
        tiling: vk::ImageTiling,
        usage: vk::ImageUsageFlags,
        aspects: vk::ImageAspectFlags,
    ) -> Result<Self> {
        let info = vk::ImageCreateInfo::builder()
            .image_type(vk::ImageType::_2D)
            .extent(vk::Extent3D {
                width: size.0,
                height: size.1,
                depth: 1,
            })
            .mip_levels(1)
            .array_layers(1)
            .format(format)
            .tiling(tiling)
            .initial_layout(vk::ImageLayout::UNDEFINED)
            .usage(usage)
            .samples(vk::SampleCountFlags::_1)
            .sharing_mode(vk::SharingMode::EXCLUSIVE);

        let image = data.device.create_image(&info, None)?;

        let requirements = data.device.get_image_memory_requirements(image);

        let alloc = data.allocator.alloc(AllocRequirements::new(
            requirements,
            AllocUsage::DeviceLocal,
        ))?;

        data.device
            .bind_image_memory(image, alloc.memory, alloc.offset)?;

        let view = create_image_view(&data.device, image, format, aspects, 1)?;

        Ok(Self {
            image,
            allocator: Arc::downgrade(&data.allocator),
            alloc,
            view,
            device: Arc::downgrade(&data.device),
        })
    }
}

impl Drop for Image {
    fn drop(&mut self) {
        let device = self.device.upgrade().unwrap();
        unsafe {
            device.destroy_image_view(self.view, None);
            self.allocator.upgrade().unwrap().free(self.alloc);
            device.destroy_image(self.image, None);
        }
    }
}
