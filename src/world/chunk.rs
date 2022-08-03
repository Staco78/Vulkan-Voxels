use std::fmt::Debug;

use anyhow::{anyhow, Result};
use log::trace;
use nalgebra_glm::{vec3, TVec3};

use crate::{
    config::CHUNK_SIZE,
    render::{buffer::Buffer, vertex::Vertex},
};

use super::world::ChunkPos;

#[derive(Debug, Clone, Copy, PartialEq)]
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

    pub fn mesh(&mut self, vertices: &mut [Vertex], indices: &mut [u32]) -> Result<()> {
        trace!("Mesh chunk {:?}", self.pos);

        // from https://github.com/fesoliveira014/cubeproject/blob/master/CubeProject/tactical/volume/mesher/ChunkMesher.cpp

        let mut vertices_index = 0;
        let mut indices_index = 0;
        let mut indices_max = 0;

        let mut emit_quad = |corners: &[TVec3<i32>; 4], side: Side| {
            let color: TVec3<u8> = vec3(255, 255, 255);
            let light_modifier = match side {
                Side::NORTH | Side::SOUTH => 8,
                Side::WEST | Side::EAST => 6,
                Side::TOP => 10,
                Side::BOTTOM => 5,
            };

            for i in 0..4 {
                vertices[vertices_index] = Vertex {
                    pos: corners[i]
                        + vec3(
                            self.pos.x * CHUNK_SIZE as i32,
                            self.pos.y as i32 * CHUNK_SIZE as i32,
                            self.pos.z * CHUNK_SIZE as i32,
                        ),
                    color,
                    light_modifier,
                };
                vertices_index += 1;
            }

            [0, 1, 2, 2, 3, 0].iter().for_each(|i| {
                indices[indices_index] = indices_max + *i as u32;
                indices_index += 1;
            });
            indices_max += 4;
        };

        #[derive(Debug, Clone, Copy)]
        enum MaskValue<'a> {
            None,
            Positive(&'a Block),
            Negative(&'a Block),
        }

        impl MaskValue<'_> {
            #[inline]
            fn is_none(&self) -> bool {
                match self {
                    Self::None => true,
                    _ => false,
                }
            }

            #[inline]
            fn is_positive(&self) -> bool {
                match self {
                    Self::Positive(_) => true,
                    _ => false,
                }
            }
        }

        impl PartialEq for MaskValue<'_> {
            fn eq(&self, other: &Self) -> bool {
                match (self, other) {
                    (Self::None, Self::None) => true,
                    (Self::Positive(a), Self::Positive(b)) => a.id == b.id,
                    (Self::Negative(a), Self::Negative(b)) => a.id == b.id,
                    _ => false,
                }
            }
        }

        let mut mask = [MaskValue::None; CHUNK_SIZE * CHUNK_SIZE];

        for axis in 0..3 {
            let u = (axis + 1) % 3;
            let v = (axis + 2) % 3;

            let mut side = Side::NORTH;

            let mut x = [0i32; 3];
            let mut q = [0i32; 3];
            q[axis] = 1;
            x[axis] = -1;

            while x[axis] < CHUNK_SIZE as i32 {
                let mut n = 0;
                for i in 0..CHUNK_SIZE {
                    x[v] = i as i32;
                    for i in 0..CHUNK_SIZE {
                        x[u] = i as i32;

                        side = Side::try_from(axis).unwrap();

                        let a = if x[axis] >= 0 {
                            if self.is_face_visible(x[0], x[1], x[2], side) {
                                let b = &self.blocks[Self::block_pos_to_index(
                                    x[0] as u32,
                                    x[1] as u32,
                                    x[2] as u32,
                                )];
                                if b.id == 0 {
                                    None
                                } else {
                                    Some(b)
                                }
                            } else {
                                None
                            }
                        } else {
                            None
                        };

                        side = Side::try_from(axis + 3).unwrap();
                        let b = if x[axis] < CHUNK_SIZE as i32 - 1 {
                            if self.is_face_visible(x[0] + q[0], x[1] + q[1], x[2] + q[2], side) {
                                let b = &self.blocks[Self::block_pos_to_index(
                                    (x[0] + q[0]) as u32,
                                    (x[1] + q[1]) as u32,
                                    (x[2] + q[2]) as u32,
                                )];
                                if b.id == 0 {
                                    None
                                } else {
                                    Some(b)
                                }
                            } else {
                                None
                            }
                        } else {
                            None
                        };

                        if a.is_some() == b.is_some() {
                            mask[n] = MaskValue::None;
                        } else if a.is_some() {
                            mask[n] = MaskValue::Positive(a.unwrap());
                        } else {
                            mask[n] = MaskValue::Negative(b.unwrap());
                        }

                        n += 1;
                    }
                }

                x[axis] += 1;
                let mut n = 0;

                for j in 0..CHUNK_SIZE {
                    let mut i = 0;
                    while i < CHUNK_SIZE {
                        let c = mask[n];
                        if !c.is_none() {
                            let mut width = 1;
                            while i + width < CHUNK_SIZE && c == mask[n + width] {
                                width += 1;
                            }

                            let mut done = false;
                            let mut height = 1;
                            while !done && height + j < CHUNK_SIZE {
                                let mut k = 0;
                                while k < width {
                                    if mask[n + k + height * CHUNK_SIZE] != c {
                                        done = true;
                                        break;
                                    }
                                    k += 1;
                                }
                                if !done {
                                    height += 1;
                                }
                            }

                            x[u] = i as i32;
                            x[v] = j as i32;
                            let mut du = [0i32; 3];
                            let mut dv = [0i32; 3];

                            if c.is_positive() {
                                dv[v] = height as i32;
                                du[u] = width as i32;
                            } else {
                                du[v] = height as i32;
                                dv[u] = width as i32;
                            }

                            emit_quad(
                                &[
                                    vec3(x[0], x[1], x[2]),
                                    vec3(x[0] + du[0], x[1] + du[1], x[2] + du[2]),
                                    vec3(
                                        x[0] + du[0] + dv[0],
                                        x[1] + du[1] + dv[1],
                                        x[2] + du[2] + dv[2],
                                    ),
                                    vec3(x[0] + dv[0], x[1] + dv[1], x[2] + dv[2]),
                                ],
                                side,
                            );

                            for l in 0..height {
                                for k in 0..width {
                                    mask[n + k + l * CHUNK_SIZE] = MaskValue::None;
                                }
                            }

                            i += width;
                            n += width;
                        } else {
                            n += 1;
                            i += 1;
                        }
                    }
                }
            }
        }

        self.vertices_count = vertices_index;
        self.indices_count = indices_index;

        Ok(())
    }

    #[inline(always)]
    fn block_pos_to_index(x: u32, y: u32, z: u32) -> usize {
        (x as usize) * CHUNK_SIZE * CHUNK_SIZE + (y as usize) * CHUNK_SIZE + (z as usize)
    }

    fn is_face_visible(&self, x: i32, y: i32, z: i32, side: Side) -> bool {
        let (x, y, z) = match side {
            Side::NORTH => (x + 1, y, z),
            Side::SOUTH => (x - 1, y, z),
            Side::EAST => (x, y, z + 1),
            Side::WEST => (x, y, z - 1),
            Side::TOP => (x, y + 1, z),
            Side::BOTTOM => (x, y - 1, z),
        };
        if x < 0
            || x >= CHUNK_SIZE as i32
            || y < 0
            || y >= CHUNK_SIZE as i32
            || z < 0
            || z >= CHUNK_SIZE as i32
        {
            return true;
        }
        let block = self.blocks[Self::block_pos_to_index(x as u32, y as u32, z as u32)];
        block.id == 0
    }
}

impl Drop for Chunk {
    fn drop(&mut self) {
        trace!("Drop chunk {:?}", self.pos);
    }
}

#[derive(Debug, Clone, Copy, PartialEq)]
enum Side {
    NORTH,  // x+
    TOP,    // y+
    EAST,   // z+
    SOUTH,  // x-
    BOTTOM, // y-
    WEST,   // z-
}

impl TryFrom<usize> for Side {
    type Error = anyhow::Error;

    fn try_from(value: usize) -> Result<Self, Self::Error> {
        match value {
            0 => Ok(Side::NORTH),
            1 => Ok(Side::TOP),
            2 => Ok(Side::EAST),
            3 => Ok(Side::SOUTH),
            4 => Ok(Side::BOTTOM),
            5 => Ok(Side::WEST),
            _ => Err(anyhow!("Invalid side")),
        }
    }
}
