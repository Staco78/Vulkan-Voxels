use std::mem::size_of;

use anyhow::Result;
use nalgebra_glm::{vec3, TVec3};
use vulkanalia::{vk, Device, Instance};

use crate::{
    config::CHUNK_SIZE,
    render::{buffer::Buffer, renderer::RendererData, vertex::Vertex},
};

use super::world::ChunkPos;

#[derive(Clone, Copy)]
pub struct Block {
    id: u16,
}

pub struct Chunk {
    pub pos: ChunkPos,
    pub blocks: [Block; CHUNK_SIZE * CHUNK_SIZE * CHUNK_SIZE],
    pub vertex_buffer: Buffer,
    pub vertices_size: usize,
}

impl Chunk {
    pub fn new(pos: ChunkPos) -> Result<Self> {
        Ok(Self {
            pos,
            blocks: [Block { id: 1 }; CHUNK_SIZE * CHUNK_SIZE * CHUNK_SIZE],
            vertex_buffer: Buffer::default(),
            vertices_size: 0,
        })
    }

    pub unsafe fn mesh(
        &mut self,
        instance: &Instance,
        device: &Device,
        renderer_data: &RendererData,
    ) -> Result<()> {
        const FRONT: [[i32; 3]; 6] = [
            [0, 0, 0],
            [0, 1, 0],
            [0, 0, 1],
            [0, 1, 0],
            [0, 1, 1],
            [0, 0, 1],
        ];
        const BACK: [[i32; 3]; 6] = [
            [1, 0, 0],
            [1, 0, 1],
            [1, 1, 0],
            [1, 1, 0],
            [1, 0, 1],
            [1, 1, 1],
        ];
        const LEFT: [[i32; 3]; 6] = [
            [1, 0, 0],
            [1, 1, 0],
            [0, 0, 0],
            [1, 1, 0],
            [0, 1, 0],
            [0, 0, 0],
        ];
        const RIGHT: [[i32; 3]; 6] = [
            [0, 0, 1],
            [0, 1, 1],
            [1, 0, 1],
            [0, 1, 1],
            [1, 1, 1],
            [1, 0, 1],
        ];
        const UP: [[i32; 3]; 6] = [
            [0, 1, 0],
            [1, 1, 0],
            [0, 1, 1],
            [1, 1, 0],
            [1, 1, 1],
            [0, 1, 1],
        ];
        const DOWN: [[i32; 3]; 6] = [
            [1, 0, 0],
            [0, 0, 0],
            [1, 0, 1],
            [0, 0, 0],
            [0, 0, 1],
            [1, 0, 1],
        ];

        fn emit_face(
            face: &[[i32; 3]; 6],
            data: &mut Vec<Vertex>,
            pos: TVec3<i32>,
            light_modifier: u8,
        ) {
            for i in 0..6 {
                let v = Vertex {
                    pos: pos + vec3(face[i][0], face[i][1], face[i][2]),
                    color: vec3(1., 1., 1.),
                    light_modifier,
                };
                data.push(v);
            }
        }

        let mut data = Vec::new();
        for x in 0..CHUNK_SIZE {
            for y in 0..CHUNK_SIZE {
                for z in 0..CHUNK_SIZE {
                    let index = x * CHUNK_SIZE * CHUNK_SIZE + y * CHUNK_SIZE + z;
                    if self.blocks[index].id != 0 {
                        let mut pos = vec3(x as i32, y as i32, z as i32);
                        pos.x += self.pos.x * CHUNK_SIZE as i32;
                        pos.y += self.pos.y as i32 * CHUNK_SIZE as i32;
                        pos.z += self.pos.z * CHUNK_SIZE as i32;

                        if x >= CHUNK_SIZE - 1
                            || self.blocks[index + CHUNK_SIZE * CHUNK_SIZE].id == 0
                        {
                            emit_face(&BACK, &mut data, pos, 8);
                        }
                        if x <= 0 || self.blocks[index - CHUNK_SIZE * CHUNK_SIZE].id == 0 {
                            emit_face(&FRONT, &mut data, pos, 8);
                        }
                        if z >= CHUNK_SIZE - 1 || self.blocks[index + 1].id == 0 {
                            emit_face(&RIGHT, &mut data, pos, 6);
                        }
                        if z <= 0 || self.blocks[index - 1].id == 0 {
                            emit_face(&LEFT, &mut data, pos, 6);
                        }
                        if y >= CHUNK_SIZE - 1 || self.blocks[index + CHUNK_SIZE].id == 0 {
                            emit_face(&UP, &mut data, pos, 10);
                        }
                        if y <= 0 || self.blocks[index - CHUNK_SIZE].id == 0 {
                            emit_face(&DOWN, &mut data, pos, 5);
                        }
                    }
                }
            }
        }

        self.vertex_buffer = Buffer::create(
            instance,
            device,
            renderer_data,
            data.len() * size_of::<Vertex>(),
            vk::BufferUsageFlags::VERTEX_BUFFER,
        )?;

        self.vertex_buffer.fill(device, data.as_ptr(), data.len())?;
        self.vertices_size = data.len();

        Ok(())
    }

    pub unsafe fn destroy(&self, device: &Device) {
        self.vertex_buffer.destroy(device);
    }
}
