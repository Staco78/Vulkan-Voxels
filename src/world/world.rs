use std::{
    collections::HashMap,
    sync::{Arc, Mutex, Weak},
};

use anyhow::Result;
use nalgebra_glm::Vec3;
use vulkanalia::vk::DeviceV1_0;

use crate::{
    config::{CHUNK_SIZE, RENDER_DISTANCE},
    render::renderer::RendererData,
    threads::MeshingThreadPool,
};

use super::Chunk;

#[derive(Debug, Clone, Copy, Hash, PartialEq, Eq)]
pub struct ChunkPos {
    pub x: i32,
    pub y: u32,
    pub z: i32,
}

pub struct World {
    pub chunks: HashMap<ChunkPos, Arc<Mutex<Chunk>>>,
    pub chunks_to_render: Vec<Weak<Mutex<Chunk>>>,
}

impl World {
    pub unsafe fn new() -> Result<Self> {
        Ok(Self {
            chunks: HashMap::new(),
            chunks_to_render: Vec::new(),
        })
    }

    #[profiling::function]
    fn update_visible_chunks(
        &mut self,
        data: &RendererData,
        meshing_pool: &MeshingThreadPool,
        player_pos: Vec3,
    ) -> Result<()> {
        let player_chunk_pos = ChunkPos {
            x: (player_pos.x / CHUNK_SIZE as f32).floor() as i32,
            y: (player_pos.y / CHUNK_SIZE as f32).floor() as u32,
            z: (player_pos.z / CHUNK_SIZE as f32).floor() as i32,
        };

        let mut chunks_to_destroy = Vec::new();
        {
            profiling::scope!("chunks_to_destroy");
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
        }

        {
            profiling::scope!("wait queues");
            unsafe {
                data.device.queue_wait_idle(data.graphics_queue)?;
                data.device.queue_wait_idle(data.present_queue)?;
            }
        }

        {
            profiling::scope!("dropping chunks");
            for pos in chunks_to_destroy {
                self.chunks.remove(&pos);
            }
        }

        {
            profiling::scope!("new chunks");
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
                        if let std::collections::hash_map::Entry::Vacant(e) = self.chunks.entry(pos)
                        {
                            let chunk = Chunk::new(pos)?;
                            let chunk = Arc::new(Mutex::new(chunk));
                            meshing_pool.mesh_thread(Arc::downgrade(&chunk));
                            e.insert(chunk);
                        }
                    }
                }
            }
        }
        {
            profiling::scope!("meshed chunks add to render");
            for chunk in meshing_pool.try_iter() {
                if chunk.upgrade().is_some() {
                    self.chunks_to_render.push(chunk);
                }
            }
        }

        Ok(())
    }

    pub fn tick(
        &mut self,
        data: &RendererData,
        meshing_pool: &MeshingThreadPool,
        player_pos: Vec3,
    ) -> Result<()> {
        self.update_visible_chunks(data, meshing_pool, player_pos)?;

        Ok(())
    }
}
