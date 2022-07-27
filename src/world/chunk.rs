use std::{collections::HashMap, fmt::Debug};

use anyhow::Result;
use log::trace;
use nalgebra_glm::{vec3, TVec3};

use crate::{
    config::CHUNK_SIZE,
    render::{buffer::Buffer, vertex::Vertex},
};

use super::world::ChunkPos;

#[derive(Debug, Clone, Copy)]
pub struct Block {
    id: u16,
}

pub struct Chunk {
    pub pos: ChunkPos,
    pub blocks: [Block; CHUNK_SIZE * CHUNK_SIZE * CHUNK_SIZE],
    pub buffer: Option<Buffer>,
    pub vertices_count: usize,
    pub indices_count: usize,
}

impl Chunk {
    #[profiling::function]
    pub fn new(pos: ChunkPos) -> Result<Self> {
        let mut c = Self {
            pos,
            blocks: [Block { id: 0 }; CHUNK_SIZE * CHUNK_SIZE * CHUNK_SIZE],
            buffer: None,
            vertices_count: 0,
            indices_count: 0,
        };

        for x in 0..CHUNK_SIZE {
            for z in 0..CHUNK_SIZE {
                let height = (x as i32 - z as i32).unsigned_abs() as usize;
                debug_assert!(height < CHUNK_SIZE);
                for y in 0..height {
                    c.blocks[x * CHUNK_SIZE * CHUNK_SIZE + y * CHUNK_SIZE + z].id = 1;
                }
                // for y in 0..CHUNK_SIZE {
                //     c.blocks[x * CHUNK_SIZE * CHUNK_SIZE + y * CHUNK_SIZE + z].id =
                //         ((x + y + z) % 2) as u16;
                // }
            }
        }

        trace!("Create chunk {:?}", c.pos);

        Ok(c)
    }

    pub fn mesh(
        &mut self,
        vertices: &mut [Vertex],
        indices: &mut [u32],
        hash_map: &mut HashMap<Vertex, u32>,
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

        let mut vertices_index = 0;
        let mut indices_index = 0;

        let mut emit_face = |face: &[[i32; 3]; 6], pos: TVec3<i32>, light_modifier: u8| {
            for vert in face.iter().take(6) {
                let v = Vertex {
                    pos: pos + vec3(vert[0], vert[1], vert[2]),
                    color: vec3(255, 255, 255),
                    light_modifier,
                };
                if let Some(index) = hash_map.get(&v) {
                    indices[indices_index] = *index;
                } else {
                    hash_map.insert(v, vertices_index as u32);
                    indices[indices_index] = vertices_index as u32;
                    vertices[vertices_index] = v;
                    vertices_index += 1;
                }
                indices_index += 1;
            }
        };

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
                            emit_face(&BACK, pos, 8);
                        }
                        if x == 0 || self.blocks[index - CHUNK_SIZE * CHUNK_SIZE].id == 0 {
                            emit_face(&FRONT, pos, 8);
                        }
                        if z >= CHUNK_SIZE - 1 || self.blocks[index + 1].id == 0 {
                            emit_face(&RIGHT, pos, 6);
                        }
                        if z == 0 || self.blocks[index - 1].id == 0 {
                            emit_face(&LEFT, pos, 6);
                        }
                        if y >= CHUNK_SIZE - 1 || self.blocks[index + CHUNK_SIZE].id == 0 {
                            emit_face(&UP, pos, 10);
                        }
                        if y == 0 || self.blocks[index - CHUNK_SIZE].id == 0 {
                            emit_face(&DOWN, pos, 5);
                        }
                    }
                }
            }
        }

        self.vertices_count = vertices_index;
        self.indices_count = indices_index;

        Ok(())
    }
}

impl Drop for Chunk {
    fn drop(&mut self) {
        trace!("Drop chunk {:?}", self.pos);
    }
}
