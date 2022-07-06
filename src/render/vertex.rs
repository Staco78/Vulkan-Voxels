use std::mem::size_of;

use nalgebra_glm as glm;
use vulkanalia::vk::{self, HasBuilder};

#[repr(C)]
#[derive(Debug)]
pub struct Vertex {
    pub pos: glm::TVec3<i32>,
    pub color: glm::Vec3,
}

impl Vertex {
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
                .format(vk::Format::R32G32B32_SINT)
                .offset(0)
                .build(),
            vk::VertexInputAttributeDescription::builder()
                .binding(0)
                .location(1)
                .format(vk::Format::R32G32B32_SFLOAT)
                .offset(size_of::<glm::TVec3<i32>>() as u32)
                .build(),
        ]
    }
}
