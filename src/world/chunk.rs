use std::mem::size_of;

use anyhow::Result;
use nalgebra_glm::{vec3, Mat4, Vec3};
use vulkanalia::{vk, Device, Instance};

use crate::{
    config::CHUNK_SIZE,
    render::{buffer::Buffer, renderer::{RendererData}, vertex::Vertex},
};

#[derive(Clone, Copy)]
pub struct Block {
    id: u16,
}

#[derive(Clone, Copy)]
pub struct ChunkPos {
    pub x: i32,
    pub y: u32,
    pub z: i32,
}

pub struct Chunk {
    pub pos: ChunkPos,
    pub blocks: [Block; CHUNK_SIZE * CHUNK_SIZE * CHUNK_SIZE],
    pub vertex_buffer: Buffer,
    pub model: Mat4,
    pub vertices_size: usize,
}

impl Chunk {
    pub fn new(pos: ChunkPos) -> Result<Self> {
        Ok(Self {
            pos,
            blocks: [Block { id: 1 }; CHUNK_SIZE * CHUNK_SIZE * CHUNK_SIZE],
            vertex_buffer: Buffer::default(),
            model: Mat4::identity(),
            vertices_size: 0,
        })
    }

    pub unsafe fn mesh(
        &mut self,
        instance: &Instance,
        device: &Device,
        renderer_data: &RendererData,
    ) -> Result<()> {
        const FRONT: [[f32; 3]; 6] = [
            [0.0, 0.0, 0.0],
            [0.0, 1.0, 0.0],
            [0.0, 0.0, 1.0],
            [0.0, 1.0, 0.0],
            [0.0, 1.0, 1.0],
            [0.0, 0.0, 1.0],
        ];
        const BACK: [[f32; 3]; 6] = [
            [1.0, 0.0, 0.0],
            [1.0, 0.0, 1.0],
            [1.0, 1.0, 0.0],
            [1.0, 1.0, 0.0],
            [1.0, 0.0, 1.0],
            [1.0, 1.0, 1.0],
        ];
        const LEFT: [[f32; 3]; 6] = [
            [1.0, 0.0, 0.0],
            [1.0, 1.0, 0.0],
            [0.0, 0.0, 0.0],
            [1.0, 1.0, 0.0],
            [0.0, 1.0, 0.0],
            [0.0, 0.0, 0.0],
        ];
        const RIGHT: [[f32; 3]; 6] = [
            [0.0, 0.0, 1.0],
            [0.0, 1.0, 1.0],
            [1.0, 0.0, 1.0],
            [0.0, 1.0, 1.0],
            [1.0, 1.0, 1.0],
            [1.0, 0.0, 1.0],
        ];
        const UP: [[f32; 3]; 6] = [
            [0.0, 1.0, 0.0],
            [1.0, 1.0, 0.0],
            [0.0, 1.0, 1.0],
            [1.0, 1.0, 0.0],
            [1.0, 1.0, 1.0],
            [0.0, 1.0, 1.0],
        ];
        const DOWN: [[f32; 3]; 6] = [
            [1.0, 0.0, 0.0],
            [0.0, 0.0, 0.0],
            [1.0, 0.0, 1.0],
            [0.0, 0.0, 0.0],
            [0.0, 0.0, 1.0],
            [1.0, 0.0, 1.0],
        ];

        fn emit_face(face: &[[f32; 3]; 6], data: &mut Vec<Vertex>, pos: Vec3) {
            for i in 0..6 {
                let v = Vertex {
                    pos: pos + vec3(face[i][0], face[i][1], face[i][2]),
                    color: vec3(1.0, 1.0, 1.0),
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
                        let pos = vec3(x as f32, y as f32, z as f32);
                        if x >= 15 || self.blocks[index + CHUNK_SIZE * CHUNK_SIZE].id == 0 {
                            emit_face(&BACK, &mut data, pos);
                        }
                        if x <= 0 || self.blocks[index - CHUNK_SIZE * CHUNK_SIZE].id == 0 {
                            emit_face(&FRONT, &mut data, pos);
                        }
                        if z >= 15 || self.blocks[index + 1].id == 0 {
                            emit_face(&RIGHT, &mut data, pos);
                        }
                        if z <= 0 || self.blocks[index - 1].id == 0 {
                            emit_face(&LEFT, &mut data, pos);
                        }
                        if y >= 15 || self.blocks[index + CHUNK_SIZE].id == 0 {
                            emit_face(&UP, &mut data, pos);
                        }
                        if y <= 0 || self.blocks[index - CHUNK_SIZE].id == 0 {
                            emit_face(&DOWN, &mut data, pos);
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
