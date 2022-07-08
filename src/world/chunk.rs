use std::mem::size_of;

use anyhow::Result;
use log::trace;
use nalgebra_glm::{vec3, TVec3};
use vulkanalia::vk;

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
    pub vertex_buffer: Option<Buffer>,
    pub vertices: Vec<Vertex>,
    pub vertices_len: usize,
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
                let height = (x as i32 - y as i32).abs() as usize;
                assert!(height < CHUNK_SIZE);
                for z in 0..height {
                    c.blocks[x * CHUNK_SIZE * CHUNK_SIZE + y * CHUNK_SIZE + z].id = 1;
                }
            }
        }

        trace!("Create chunk {:?}", c.pos);

        Ok(c)
    }

    pub fn mesh(&mut self) -> Result<()> {
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
                            emit_face(&BACK, &mut self.vertices, pos, 8);
                        }
                        if x <= 0 || self.blocks[index - CHUNK_SIZE * CHUNK_SIZE].id == 0 {
                            emit_face(&FRONT, &mut self.vertices, pos, 8);
                        }
                        if z >= CHUNK_SIZE - 1 || self.blocks[index + 1].id == 0 {
                            emit_face(&RIGHT, &mut self.vertices, pos, 6);
                        }
                        if z <= 0 || self.blocks[index - 1].id == 0 {
                            emit_face(&LEFT, &mut self.vertices, pos, 6);
                        }
                        if y >= CHUNK_SIZE - 1 || self.blocks[index + CHUNK_SIZE].id == 0 {
                            emit_face(&UP, &mut self.vertices, pos, 10);
                        }
                        if y <= 0 || self.blocks[index - CHUNK_SIZE].id == 0 {
                            emit_face(&DOWN, &mut self.vertices, pos, 5);
                        }
                    }
                }
            }
        }

        Ok(())
    }

    pub unsafe fn finish_mesh(&mut self, renderer_data: &RendererData) -> Result<()> {
        trace!("Finish mesh chunk {:?}", self.pos);
        
        self.vertex_buffer = Some(Buffer::create(
            renderer_data,
            self.vertices.len() * size_of::<Vertex>(),
            vk::BufferUsageFlags::VERTEX_BUFFER,
        )?);

        self.vertex_buffer.as_mut().unwrap().fill(
            &renderer_data.device,
            self.vertices.as_ptr(),
            self.vertices.len(),
        )?;

        self.vertices_len = self.vertices.len();

        self.vertices.clear();
        self.vertices.shrink_to_fit();
        Ok(())
    }
}

impl Drop for Chunk {
    fn drop(&mut self) {
        trace!("Drop chunk {:?}", self.pos);
    }
}