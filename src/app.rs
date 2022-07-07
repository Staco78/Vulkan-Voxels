use crate::{inputs::Inputs, render::Renderer, world::World};
use anyhow::Result;
use vulkanalia::{vk::DeviceV1_0, Entry};
use winit::window::Window;

pub struct App {
    pub world: World,
    pub renderer: Renderer,
    pub inputs: Inputs,
}

impl App {
    pub fn create(window: &Window, entry: &Entry) -> Result<Self> {
        let renderer = unsafe { Renderer::new(window, entry)? };
        let world = World::new();
        Ok(Self {
            renderer,
            world,
            inputs: Inputs::new(),
        })
    }

    pub fn tick(&mut self) -> Result<()> {
        self.world.tick(
            &self.renderer.instance,
            &self.renderer.device,
            &self.renderer.data,
            self.renderer.camera.pos,
        )?;
        unsafe { self.renderer.record_commands(&self.world)? };
        Ok(())
    }

    pub fn update(&mut self, dt: f32) -> Result<()> {
        unsafe { self.renderer.update(&self.inputs, dt) }
    }

    pub fn render(&mut self, window: &Window, dt: f32) -> Result<()> {
        unsafe {
            self.renderer.render(window, &self.world, dt)?;
        }
        Ok(())
    }
}

impl Drop for App {
    fn drop(&mut self) {
        unsafe {
            self.renderer.device.device_wait_idle().unwrap();
            for chunk in self.world.chunks.values() {
                chunk.destroy(&self.renderer.device);
            }
        }
    }
}
