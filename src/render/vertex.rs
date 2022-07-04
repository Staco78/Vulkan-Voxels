use std::mem::size_of;

use nalgebra_glm as glm;
use vulkanalia::vk::{self, HasBuilder};

#[repr(C)]
pub struct Vertex {
    pub position: glm::Vec3,
    pub color: glm::Vec3,
}

impl Vertex {
    pub fn new(position: glm::Vec3, color: glm::Vec3) -> Self {
        Self { position, color }
    }
    
    pub fn binding_description() -> vk::VertexInputBindingDescription {
        vk::VertexInputBindingDescription::builder()
            .binding(0)
            .stride(size_of::<Self>() as u32)
            .input_rate(vk::VertexInputRate::VERTEX)
            .build()
    }

    pub fn attribute_descriptions() -> [vk::VertexInputAttributeDescription; 2] {
        [
            vk::VertexInputAttributeDescription::builder()
                .binding(0)
                .location(0)
                .format(vk::Format::R32G32B32_SFLOAT)
                .offset(0)
                .build(),
            vk::VertexInputAttributeDescription::builder()
                .binding(0)
                .location(1)
                .format(vk::Format::R32G32B32_SFLOAT)
                .offset(size_of::<glm::Vec3>() as u32)
                .build(),
        ]
    }
}
