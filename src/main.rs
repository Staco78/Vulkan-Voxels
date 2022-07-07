mod app;
mod config;
mod inputs;
mod render;
mod world;

use std::time::Instant;

use app::App;
use log::warn;
use vulkanalia::{
    loader::{LibloadingLoader, LIBRARY},
    Entry,
};
use winit::{
    dpi::LogicalSize,
    event::Event,
    event::{DeviceEvent, VirtualKeyCode, WindowEvent},
    event_loop::{ControlFlow, EventLoop},
    window::{Fullscreen, WindowBuilder},
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

    window.set_cursor_grab(true).unwrap();
    window.set_cursor_visible(false);
    let mut last_frame_time = Instant::now();

    event_loop.run(move |event, _, control_flow| {
        *control_flow = ControlFlow::Poll;

        match event {
            Event::WindowEvent {
                event: WindowEvent::KeyboardInput { input, .. },
                ..
            } => {
                if let Some(key) = input.virtual_keycode {
                    if key == VirtualKeyCode::F11
                        && input.state == winit::event::ElementState::Pressed
                    {
                        if window.fullscreen().is_some() {
                            window.set_fullscreen(None);
                        } else {
                            window.set_fullscreen(Some(Fullscreen::Borderless(None)));
                        }
                    }
                    if input.state == winit::event::ElementState::Pressed {
                        app.inputs.key_pressed(key);
                    } else {
                        app.inputs.key_released(key);
                    }
                } else {
                    warn!("Unknown key pressed: {:?}", input);
                }
            }
            Event::DeviceEvent {
                event: DeviceEvent::MouseMotion { delta },
                ..
            } => {
                app.inputs.mouse_moved(delta);
            }
            Event::WindowEvent {
                event: WindowEvent::Focused(focused),
                ..
            } => {
                if focused {
                    window.set_cursor_grab(true).unwrap();
                    window.set_cursor_visible(false);
                } else {
                    window.set_cursor_grab(false).unwrap();
                    window.set_cursor_visible(true);
                }
            }
            Event::MainEventsCleared => {
                let dt = last_frame_time.elapsed().as_secs_f32();
                last_frame_time = Instant::now();

                app.tick().unwrap();
                app.update(dt).unwrap();
                app.render(&window, dt).unwrap();
                app.inputs.reset();
            }
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
