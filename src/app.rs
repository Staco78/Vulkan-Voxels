use crate::{inputs::Inputs, render::Renderer};
use anyhow::Result;
use vulkanalia::Entry;
use winit::window::Window;

pub struct App {
    pub renderer: Renderer,
    pub inputs: Inputs,
}

impl App {
    pub fn create(window: &Window, entry: &Entry) -> Result<Self> {
        let mut renderer = unsafe { Renderer::new(window, entry)? };
        unsafe { renderer.record_commands()? };
        Ok(Self {
            renderer,
            inputs: Inputs::new(),
        })
    }

    pub fn update(&mut self, dt: f32) -> Result<()> {
        unsafe { self.renderer.update(&self.inputs, dt) }
    }

    pub fn render(&mut self, window: &Window, dt: f32) -> Result<()> {
        unsafe {
            self.renderer.render(window, dt)?;
        }
        Ok(())
    }
}
