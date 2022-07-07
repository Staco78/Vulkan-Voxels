use std::collections::HashMap;

use anyhow::Result;
use nalgebra_glm::Vec3;
use vulkanalia::{vk::DeviceV1_0, Device, Instance};

use crate::{
    config::{CHUNK_SIZE, RENDER_DISTANCE},
    render::renderer::RendererData,
};

use super::Chunk;

#[derive(Debug, Clone, Copy, Hash, PartialEq, Eq)]
pub struct ChunkPos {
    pub x: i32,
    pub y: u32,
    pub z: i32,
}

pub struct World {
    pub chunks: HashMap<ChunkPos, Chunk>,
}

impl World {
    pub fn new() -> Self {
        Self {
            chunks: HashMap::new(),
        }
    }

    fn update_visible_chunks(
        &mut self,
        instance: &Instance,
        device: &Device,
        data: &RendererData,
        player_pos: Vec3,
    ) -> Result<()> {
        let player_chunk_pos = ChunkPos {
            x: (player_pos.x / CHUNK_SIZE as f32).floor() as i32,
            y: (player_pos.y / CHUNK_SIZE as f32).floor() as u32,
            z: (player_pos.z / CHUNK_SIZE as f32).floor() as i32,
        };

        let mut chunks_to_destroy = Vec::new();
        for pos in self.chunks.keys() {
            if (pos.x - player_chunk_pos.x).abs() > (RENDER_DISTANCE + 2) as i32 {
                chunks_to_destroy.push(*pos);
            }
            if (pos.y as i32 - player_chunk_pos.y as i32).abs() > (RENDER_DISTANCE + 2) as i32 {
                chunks_to_destroy.push(*pos);
            }
            if (pos.z - player_chunk_pos.z).abs() > (RENDER_DISTANCE + 2) as i32 {
                chunks_to_destroy.push(*pos);
            }
        }

        unsafe {
            device.device_wait_idle()?;
        }

        for pos in chunks_to_destroy {
            unsafe {
                self.chunks.get(&pos).unwrap().destroy(device);
            }
            self.chunks.remove(&pos);
        }

        for x in (player_chunk_pos.x - RENDER_DISTANCE as i32)
            ..(player_chunk_pos.x + RENDER_DISTANCE as i32)
        {
            for y in (player_chunk_pos.y as i32 - RENDER_DISTANCE as i32)
                ..(player_chunk_pos.y as i32 + RENDER_DISTANCE as i32)
            {
                if y < 0 || y > 10 {
                    continue;
                }
                for z in (player_chunk_pos.z - RENDER_DISTANCE as i32)
                    ..(player_chunk_pos.z + RENDER_DISTANCE as i32)
                {
                    let pos = ChunkPos { x, y: y as u32, z };
                    if !self.chunks.contains_key(&pos) {
                        let mut chunk = Chunk::new(pos)?;
                        unsafe { chunk.mesh(instance, device, data)? };
                        self.chunks.insert(pos, chunk);
                    }
                }
            }
        }

        Ok(())
    }

    pub fn tick(
        &mut self,
        instance: &Instance,
        device: &Device,
        data: &RendererData,
        player_pos: Vec3,
    ) -> Result<()> {
        self.update_visible_chunks(instance, device, data, player_pos)?;

        Ok(())
    }
}
