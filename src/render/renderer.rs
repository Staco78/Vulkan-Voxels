use std::{
    cell::RefCell,
    sync::{Arc, Mutex, RwLock, Weak},
};

use anyhow::{anyhow, Result};
use log::{debug, trace};
use nalgebra_glm as glm;
use vulkanalia::{
    self,
    vk::{self, DeviceV1_0, Handle, HasBuilder, KhrSurfaceExtension, KhrSwapchainExtension},
    Device, Entry, Instance,
};
use winit::window::Window;

use crate::{config::MAX_FRAMES_IN_FLIGHT, inputs::Inputs, world::Chunk};

use super::{
    camera::Camera,
    commands::{CommandBuffer, CommandPool},
    depth::DepthBuffer,
    device,
    framebuffers::Framebuffers,
    instance,
    memory::Allocator,
    physical_device::PhysicalDevice,
    pipeline::Pipeline,
    swapchain::Swapchain,
    sync,
    uniforms::Uniforms,
};

#[repr(C)]
#[derive(Debug, Clone, Copy)]
pub struct UniformBufferObject {
    pub view: glm::Mat4,
    pub proj: glm::Mat4,
}

pub struct Renderer {
    pub data: Arc<RwLock<RendererData>>,
    frame: usize,
    pub resized: bool,
    pub camera: RefCell<Camera>,
}

impl Renderer {
    pub unsafe fn new(window: &Window, entry: &Entry) -> Self {
        let (instance, messenger) = instance::create(window, entry).unwrap();
        let surface = vulkanalia::window::create_surface(&instance, window).unwrap();
        let physical_device = PhysicalDevice::pick(&instance, surface).unwrap();
        let (device, graphics_queue, present_queue) =
            device::create(&instance, &physical_device).unwrap();
        let device = Arc::new(device);

        let allocator = Arc::new(Allocator::new(&device, &instance, physical_device.device));

        let mut data = RendererData::new(
            instance,
            messenger,
            surface,
            physical_device,
            device,
            graphics_queue,
            present_queue,
            allocator,
        );

        data.swapchain = Some(Swapchain::create(window, &data).unwrap());
        data.uniforms = Some(Uniforms::create(&data).unwrap());
        data.depth_buffer = Some(DepthBuffer::create(&data).unwrap());
        data.pipeline = Some(Pipeline::create(&data).unwrap());
        data.framebuffers = Some(Framebuffers::create(&data).unwrap());
        data.command_pool =
            Some(CommandPool::create(&data, data.physical_device.graphics_queue.family).unwrap());
        data.command_buffers = data
            .command_pool
            .as_mut()
            .unwrap()
            .allocate_command_buffers(
                &data.device,
                data.swapchain.as_ref().unwrap().images.len() as u32,
            )
            .unwrap()
            .iter()
            .map(|b| Mutex::new(*b))
            .collect();

        let camera = RefCell::new(Camera::new(&mut data).unwrap());

        Renderer::create_sync_objects(&mut data).unwrap();

        Self {
            data: Arc::new(RwLock::new(data)),
            frame: 0,
            resized: false,
            camera,
        }
    }

    unsafe fn create_sync_objects(data: &mut RendererData) -> Result<()> {
        data.image_available_semaphore =
            sync::create_semaphores(&data.device, MAX_FRAMES_IN_FLIGHT)?;
        data.render_finished_semaphore =
            sync::create_semaphores(&data.device, MAX_FRAMES_IN_FLIGHT)?;
        data.in_flight_fences = sync::create_fences(&data.device, true, MAX_FRAMES_IN_FLIGHT)?;
        data.images_in_flight = Mutex::new(
            data.swapchain
                .as_ref()
                .unwrap()
                .images
                .iter()
                .map(|_| vk::Fence::null())
                .collect(),
        );
        Ok(())
    }

    unsafe fn destroy_sync_objects(data: &mut RendererData) -> Result<()> {
        data.image_available_semaphore
            .iter()
            .for_each(|s| data.device.destroy_semaphore(*s, None));
        data.image_available_semaphore.clear();

        data.render_finished_semaphore
            .iter()
            .for_each(|s| data.device.destroy_semaphore(*s, None));
        data.render_finished_semaphore.clear();

        data.in_flight_fences
            .iter()
            .for_each(|f| data.device.destroy_fence(*f, None));
        data.in_flight_fences.clear();

        Ok(())
    }

    #[profiling::function]
    pub unsafe fn record_commands(
        &self,
        chunks: &mut Vec<Weak<Mutex<Chunk>>>,
        image_index: usize,
    ) -> Result<()> {
        let t = std::time::Instant::now();
        debug!("Recording commands");

        let data = self.data.read().unwrap();
        let command_buffer = &mut data.command_buffers[image_index].lock().unwrap();

        command_buffer.begin(&data.device)?;

        let render_area = vk::Rect2D::builder()
            .offset(vk::Offset2D::default())
            .extent(data.swapchain.as_ref().unwrap().extent);

        let color_clear_value = vk::ClearValue {
            color: vk::ClearColorValue {
                float32: [0.0, 0.0, 0.0, 1.0],
            },
        };

        let depth_clear_value = vk::ClearValue {
            depth_stencil: vk::ClearDepthStencilValue {
                depth: 1.0,
                stencil: 0,
            },
        };

        let clear_values = &[color_clear_value, depth_clear_value];
        let info = vk::RenderPassBeginInfo::builder()
            .render_pass(data.pipeline.as_ref().unwrap().render_pass)
            .framebuffer(data.framebuffers.as_ref().unwrap()[image_index])
            .render_area(render_area)
            .clear_values(clear_values);

        data.device.cmd_begin_render_pass(
            command_buffer.buffer,
            &info,
            vk::SubpassContents::INLINE,
        );
        data.device.cmd_bind_pipeline(
            command_buffer.buffer,
            vk::PipelineBindPoint::GRAPHICS,
            data.pipeline.as_ref().unwrap().pipeline,
        );

        data.device.cmd_bind_descriptor_sets(
            command_buffer.buffer,
            vk::PipelineBindPoint::GRAPHICS,
            data.pipeline.as_ref().unwrap().layout,
            0,
            &[data.uniforms.as_ref().unwrap().descriptor_sets[image_index]],
            &[],
        );

        let mut to_remove = Vec::new();

        for (i, chunk) in chunks.iter().enumerate() {
            if let Some(chunk) = chunk.upgrade() {
                let chunk = chunk.lock().unwrap();
                data.device.cmd_bind_vertex_buffers(
                    command_buffer.buffer,
                    0,
                    &[chunk
                        .vertex_buffer
                        .as_ref()
                        .expect("Chunk not meshed")
                        .buffer],
                    &[0],
                );

                data.device
                    .cmd_draw(command_buffer.buffer, chunk.vertices_len as u32, 1, 0, 0);
            } else {
                to_remove.push(i);
            }
        }

        to_remove.reverse();

        for i in to_remove {
            chunks.swap_remove(i);
        }

        data.device.cmd_end_render_pass(command_buffer.buffer);

        command_buffer.end(&data.device)?;

        debug!("Recording commands took {:?}", t.elapsed());
        Ok(())
    }

    pub unsafe fn update(&mut self, inputs: &Inputs, dt: f32) -> Result<()> {
        self.camera.get_mut().update(inputs, dt);
        Ok(())
    }

    #[profiling::function]
    pub unsafe fn render(
        &mut self,
        window: &Window,
        chunks: &mut Vec<Weak<Mutex<Chunk>>>,
        _dt: f32,
    ) -> Result<()> {
        let data = self.data.read().unwrap();
        data.device.wait_for_fences(
            &[data.in_flight_fences[self.frame]],
            true,
            u64::max_value(),
        )?;

        let result = data.device.acquire_next_image_khr(
            data.swapchain.as_ref().unwrap().swapchain,
            u64::max_value(),
            data.image_available_semaphore[self.frame],
            vk::Fence::null(),
        );

        let image_index = match result {
            Ok((image_index, _)) => image_index as usize,
            Err(vk::ErrorCode::OUT_OF_DATE_KHR) => {
                drop(data); // deadlock if we don't drop the read lock
                return self.recreate_swapchain(window);
            }
            Err(e) => return Err(anyhow!(e)),
        };

        {
            profiling::scope!("wait imge in flight");
            let mut images_in_flight = data.images_in_flight.lock().unwrap();

            if !images_in_flight[image_index as usize].is_null() {
                data.device.wait_for_fences(
                    &[images_in_flight[image_index as usize]],
                    true,
                    u64::max_value(),
                )?;
            }

            images_in_flight[image_index as usize] = data.in_flight_fences[self.frame];
        }

        self.camera.get_mut().send(&data, image_index)?;
        self.record_commands(chunks, image_index)?;

        let wait_semaphores = &[data.image_available_semaphore[self.frame]];
        let wait_stages = &[vk::PipelineStageFlags::COLOR_ATTACHMENT_OUTPUT];
        let command_buffers = &[data.command_buffers[image_index as usize]
            .lock()
            .unwrap()
            .buffer];
        let signal_semaphores = &[data.render_finished_semaphore[self.frame]];
        let submit_info = vk::SubmitInfo::builder()
            .wait_semaphores(wait_semaphores)
            .wait_dst_stage_mask(wait_stages)
            .command_buffers(command_buffers)
            .signal_semaphores(signal_semaphores);

        data.device
            .reset_fences(&[data.in_flight_fences[self.frame]])?;

        data.device.queue_submit(
            data.graphics_queue,
            &[submit_info],
            data.in_flight_fences[self.frame],
        )?;

        let swapchains = &[data.swapchain.as_ref().unwrap().swapchain];
        let image_indices = &[image_index as u32];
        let present_info = vk::PresentInfoKHR::builder()
            .wait_semaphores(signal_semaphores)
            .swapchains(swapchains)
            .image_indices(image_indices);

        let result = data
            .device
            .queue_present_khr(data.present_queue, &present_info);

        let changed = result == Ok(vk::SuccessCode::SUBOPTIMAL_KHR)
            || result == Err(vk::ErrorCode::OUT_OF_DATE_KHR);

        if changed || self.resized {
            drop(data); // deadlock if we don't drop the read lock
            self.recreate_swapchain(window)?;
            self.resized = false;
        } else if let Err(e) = result {
            return Err(anyhow!(e));
        }

        self.frame = (self.frame + 1) % MAX_FRAMES_IN_FLIGHT;

        profiling::finish_frame!();

        Ok(())
    }

    pub unsafe fn recreate_swapchain(&self, window: &Window) -> Result<()> {
        trace!("Recreating swapchain");

        let mut data = self.data.write().unwrap();

        data.device.queue_wait_idle(data.graphics_queue)?;
        data.device.queue_wait_idle(data.present_queue)?;

        data.uniforms = None;
        data.depth_buffer = None;
        data.framebuffers = None;
        data.device.free_command_buffers(
            data.command_pool.as_ref().unwrap().pool,
            &data
                .command_buffers
                .iter()
                .map(|b| b.lock().unwrap().buffer)
                .collect::<Vec<vk::CommandBuffer>>(),
        );
        data.command_buffers.clear();
        data.pipeline = None;
        data.swapchain = None;
        data.swapchain = Some(Swapchain::create(window, &data)?);
        data.uniforms = Some(Uniforms::create(&data)?);
        data.depth_buffer = Some(DepthBuffer::create(&data)?);
        data.pipeline = Some(Pipeline::create(&data)?);
        data.framebuffers = Some(Framebuffers::create(&data)?);
        let swapchain_len = data.swapchain.as_ref().unwrap().images.len();
        data.command_buffers = data
            .command_pool
            .as_ref()
            .unwrap()
            .allocate_command_buffers(&data.device, swapchain_len as u32)?
            .iter()
            .map(|b| Mutex::new(*b))
            .collect();
        data.images_in_flight
            .get_mut()
            .unwrap()
            .resize(swapchain_len, vk::Fence::null());
        self.camera.borrow_mut().update_projection(&data);
        self.camera.borrow().send_all(&data)?;

        Ok(())
    }
}

impl Drop for Renderer {
    fn drop(&mut self) {
        unsafe {
            let mut data = self.data.write().unwrap();
            data.device.device_wait_idle().unwrap();

            // set all options to None to call Drop in the right order
            data.depth_buffer = None;
            data.uniforms = None;
            data.framebuffers = None;
            data.command_buffers.clear();
            data.command_pool = None;
            data.pipeline = None;
            data.swapchain = None;

            Arc::get_mut(&mut data.allocator).unwrap().free_all();

            Renderer::destroy_sync_objects(&mut data).unwrap();

            device::destroy(&mut data.device);
            data.instance.destroy_surface_khr(data.surface, None);
            instance::destroy(&mut data);
        }
    }
}

pub struct RendererData {
    pub instance: Instance,
    pub messenger: Option<vk::DebugUtilsMessengerEXT>,
    pub surface: vk::SurfaceKHR,
    pub physical_device: PhysicalDevice,
    pub device: Arc<Device>,
    pub graphics_queue: vk::Queue,
    pub present_queue: vk::Queue,
    pub allocator: Arc<Allocator>,
    pub swapchain: Option<Swapchain>,
    pub pipeline: Option<Pipeline>,
    pub framebuffers: Option<Framebuffers>,
    pub command_pool: Option<CommandPool>,
    pub command_buffers: Vec<Mutex<CommandBuffer>>,
    pub image_available_semaphore: Vec<vk::Semaphore>,
    pub render_finished_semaphore: Vec<vk::Semaphore>,
    pub in_flight_fences: Vec<vk::Fence>,
    pub images_in_flight: Mutex<Vec<vk::Fence>>,
    pub uniforms: Option<Uniforms<UniformBufferObject>>,
    pub depth_buffer: Option<DepthBuffer>,
}

impl RendererData {
    pub fn new(
        instance: Instance,
        messenger: Option<vk::DebugUtilsMessengerEXT>,
        surface: vk::SurfaceKHR,
        physical_device: PhysicalDevice,
        device: Arc<Device>,
        graphics_queue: vk::Queue,
        present_queue: vk::Queue,
        allocator: Arc<Allocator>,
    ) -> Self {
        Self {
            instance,
            messenger,
            surface,
            physical_device,
            device,
            graphics_queue,
            present_queue,
            allocator,
            swapchain: None,
            pipeline: None,
            framebuffers: None,
            command_pool: None,
            command_buffers: Vec::new(),
            image_available_semaphore: Vec::new(),
            render_finished_semaphore: Vec::new(),
            in_flight_fences: Vec::new(),
            images_in_flight: Mutex::new(Vec::new()),
            uniforms: None,
            depth_buffer: None,
        }
    }
}
