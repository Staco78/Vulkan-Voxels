use std::{fmt::Debug, mem::size_of};

use anyhow::Result;
use log::trace;
use nalgebra_glm::{vec3, TVec3};
use vulkanalia::vk;

use crate::{
    config::CHUNK_SIZE,
    render::{buffer::Buffer, memory::AllocUsage, renderer::RendererData, vertex::Vertex},
};

use super::world::ChunkPos;

#[derive(Debug, Clone, Copy)]
pub struct Block {
    id: u16,
}

pub struct Chunk {
    pub pos: ChunkPos,
    pub blocks: [Block; CHUNK_SIZE * CHUNK_SIZE * CHUNK_SIZE],
    pub vertex_buffer: Option<Buffer>,
    pub vertices: Vec<Vertex>,
    pub vertices_len: usize,
}

impl Debug for Chunk {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        // write!(f, "Chunk {{ pos: {:?}, vertex_buffer: {:?}, vertices: {:?}, vertices_len: {:?} }}", self.pos, self.vertex_buffer, self.vertices, self.vertices_len)
        f.debug_struct("Chunk")
            .field("pos", &self.pos)
            .field("vertex_buffer", &self.vertex_buffer)
            .field("vertices", &self.vertices)
            .field("vertices_len", &self.vertices_len)
            .finish()
    }
}

impl Chunk {
    pub fn new(pos: ChunkPos) -> Result<Self> {
        let mut c = Self {
            pos,
            blocks: [Block { id: 0 }; CHUNK_SIZE * CHUNK_SIZE * CHUNK_SIZE],
            vertex_buffer: None,
            vertices: Vec::new(),
            vertices_len: 0,
        };

        for x in 0..CHUNK_SIZE {
            for y in 0..CHUNK_SIZE {
                let height = (x as i32 - y as i32).unsigned_abs() as usize;
                assert!(height < CHUNK_SIZE);
                for z in 0..height {
                    c.blocks[x * CHUNK_SIZE * CHUNK_SIZE + y * CHUNK_SIZE + z].id = 1;
                }
            }
        }

        trace!("Create chunk {:?}", c.pos);

        Ok(c)
    }

    pub unsafe fn mesh(
        &mut self,
        renderer_data: &RendererData,
        staging_buffer: *mut Vertex,
    ) -> Result<()> {
        trace!("Mesh chunk {:?}", self.pos);

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

        unsafe fn emit_face(
            face: &[[i32; 3]; 6],
            data: *mut Vertex,
            pos: TVec3<i32>,
            light_modifier: u8,
        ) {
            let mut data = data;
            for vert in face.iter().take(6) {
                let v = Vertex {
                    pos: pos + vec3(vert[0], vert[1], vert[2]),
                    color: vec3(1., 1., 1.),
                    light_modifier,
                };
                *data = v;
                data = data.add(1);
            }
        }

        let mut i = 0;

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
                            emit_face(&BACK, staging_buffer.offset(i), pos, 8);
                            i += 6;
                        }
                        if x == 0 || self.blocks[index - CHUNK_SIZE * CHUNK_SIZE].id == 0 {
                            emit_face(&FRONT, staging_buffer.offset(i), pos, 8);
                            i += 6;
                        }
                        if z >= CHUNK_SIZE - 1 || self.blocks[index + 1].id == 0 {
                            emit_face(&RIGHT, staging_buffer.offset(i), pos, 6);
                            i += 6;
                        }
                        if z == 0 || self.blocks[index - 1].id == 0 {
                            emit_face(&LEFT, staging_buffer.offset(i), pos, 6);
                            i += 6;
                        }
                        if y >= CHUNK_SIZE - 1 || self.blocks[index + CHUNK_SIZE].id == 0 {
                            emit_face(&UP, staging_buffer.offset(i), pos, 10);
                            i += 6;
                        }
                        if y == 0 || self.blocks[index - CHUNK_SIZE].id == 0 {
                            emit_face(&DOWN, staging_buffer.offset(i), pos, 5);
                            i += 6;
                        }
                    }
                }
            }
        }

        self.vertex_buffer = Some(Buffer::create(
            renderer_data,
            i as usize * size_of::<Vertex>(),
            vk::BufferUsageFlags::VERTEX_BUFFER | vk::BufferUsageFlags::TRANSFER_DST,
            AllocUsage::DeviceLocal,
        )?);

        self.vertices_len = i as usize;

        Ok(())
    }
}

impl Drop for Chunk {
    fn drop(&mut self) {
        trace!("Drop chunk {:?}", self.pos);
    }
}
