use crate::{inputs::Inputs, render::Renderer, threads::MeshingThreadPool, world::World};
use anyhow::Result;
use vulkanalia::{vk::DeviceV1_0, Entry};
use winit::window::Window;

pub struct App {
    pub world: World,
    pub renderer: Renderer,
    pub inputs: Inputs,

    pub meshing_threads: MeshingThreadPool,
}

impl App {
    pub fn create(window: &Window, entry: &Entry) -> Result<Self> {
        let renderer = unsafe { Renderer::new(window, entry) };
        let world = unsafe { World::new()? };
        let mut thread_pool = MeshingThreadPool::new();
        unsafe { thread_pool.start_threads(renderer.data.clone()) };
        Ok(Self {
            renderer,
            world,
            inputs: Inputs::new(),
            meshing_threads: thread_pool,
        })
    }

    pub fn tick(&mut self) -> Result<()> {
        self.world.tick(
            &self.renderer.data.read().unwrap(),
            &self.meshing_threads,
            self.renderer.camera.borrow().pos,
        )?;
        Ok(())
    }

    pub fn update(&mut self, dt: f32) -> Result<()> {
        unsafe { self.renderer.update(&self.inputs, dt) }
    }

    pub fn render(&mut self, window: &Window, dt: f32) -> Result<()> {
        unsafe {
            self.renderer
                .render(window, &mut self.world.chunks_to_render, dt)?;
        }
        Ok(())
    }
}

impl Drop for App {
    fn drop(&mut self) {
        self.meshing_threads.exit_all();
        unsafe {
            self.renderer
                .data
                .read()
                .unwrap()
                .device
                .device_wait_idle()
                .unwrap();
        }
    }
}
