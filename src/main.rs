mod app;
mod config;
mod render;

use app::App;
use vulkanalia::{
    loader::{LibloadingLoader, LIBRARY},
    Entry,
};
use winit::{
    dpi::LogicalSize,
    event::Event,
    event::WindowEvent,
    event_loop::{ControlFlow, EventLoop},
    window::WindowBuilder,
};

fn main() {
    pretty_env_logger::init();

    let event_loop = EventLoop::new();
    let window = WindowBuilder::new()
        .with_title("Vulkan Voxels")
        .with_inner_size(LogicalSize::new(1080, 720))
        .build(&event_loop)
        .unwrap();

    let loader = unsafe { LibloadingLoader::new(LIBRARY) }.unwrap();
    let entry = unsafe { Entry::new(loader) }.unwrap();

    let mut app = App::create(&window, &entry).unwrap();

    event_loop.run(move |event, _, control_flow| {
        *control_flow = ControlFlow::Poll;

        match event {
            Event::MainEventsCleared => app.render(&window).unwrap(),
            Event::WindowEvent {
                event: WindowEvent::CloseRequested,
                ..
            } => *control_flow = ControlFlow::Exit,
            Event::WindowEvent {
                event: WindowEvent::Resized(_),
                ..
            } => app.renderer.resized = true,
            _ => {}
        }
    });
}
