use crate::render::Renderer;
use anyhow::Result;
use vulkanalia::Entry;
use winit::window::Window;

pub struct App {
    pub renderer: Renderer,
}

impl App {
    pub fn create(window: &Window, entry: &Entry) -> Result<Self> {
        let mut renderer = unsafe { Renderer::new(window, entry)? };
        unsafe { renderer.record_commands()? };
        Ok(Self { renderer })
    }

    pub fn render(&mut self, window: &Window) -> Result<()> {
        unsafe {
            self.renderer.render(window)?;
        }
        Ok(())
    }
}
