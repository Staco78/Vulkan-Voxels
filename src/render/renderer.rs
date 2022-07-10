use std::sync::{Arc, Mutex, Weak};

use anyhow::{anyhow, Result};
use log::debug;
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
    physical_device,
    pipeline::Pipeline,
    swapchain::Swapchain,
    sync,
    uniforms::Uniforms,
};

#[repr(C)]
pub struct UniformBufferObject {
    pub view: glm::Mat4,
    pub proj: glm::Mat4,
}

pub struct Renderer {
    pub data: RendererData,
    frame: usize,
    pub resized: bool,
    pub camera: Camera,
}

impl Renderer {
    pub unsafe fn new(window: &Window, entry: &Entry) -> Self {
        let (instance, messenger) = instance::create(window, entry).unwrap();
        let surface = vulkanalia::window::create_surface(&instance, window).unwrap();
        let physical_device = physical_device::pick(&instance, surface).unwrap();
        let (device, graphics_queue, present_queue) =
            device::create(&instance, surface, physical_device).unwrap();
        let device = Arc::new(device);

        let allocator = Arc::new(Allocator::new(&device, &instance, physical_device));

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
        data.command_pool = Some(CommandPool::create(&data).unwrap());
        data.command_buffers = data
            .command_pool
            .as_mut()
            .unwrap()
            .allocate_command_buffers(
                &data.device,
                data.swapchain.as_ref().unwrap().images.len() as u32,
            )
            .unwrap();

        let camera = Camera::new(&mut data).unwrap();

        let mut r = Self {
            data,
            frame: 0,
            resized: false,
            camera,
        };

        r.create_sync_objects().unwrap();

        r
    }

    unsafe fn create_sync_objects(&mut self) -> Result<()> {
        self.data.image_available_semaphore =
            sync::create_semaphores(&self.data.device, MAX_FRAMES_IN_FLIGHT)?;
        self.data.render_finished_semaphore =
            sync::create_semaphores(&self.data.device, MAX_FRAMES_IN_FLIGHT)?;
        self.data.in_flight_fences =
            sync::create_fences(&self.data.device, true, MAX_FRAMES_IN_FLIGHT)?;
        self.data.images_in_flight = self
            .data
            .swapchain
            .as_ref()
            .unwrap()
            .images
            .iter()
            .map(|_| vk::Fence::null())
            .collect();
        Ok(())
    }

    unsafe fn destroy_sync_objects(&mut self) -> Result<()> {
        self.data
            .image_available_semaphore
            .iter()
            .for_each(|s| self.data.device.destroy_semaphore(*s, None));
        self.data.image_available_semaphore.clear();

        self.data
            .render_finished_semaphore
            .iter()
            .for_each(|s| self.data.device.destroy_semaphore(*s, None));
        self.data.render_finished_semaphore.clear();

        self.data
            .in_flight_fences
            .iter()
            .for_each(|f| self.data.device.destroy_fence(*f, None));
        self.data.in_flight_fences.clear();

        Ok(())
    }

    pub unsafe fn record_commands(
        &mut self,
        chunks: &mut Vec<Weak<Mutex<Chunk>>>,
        image_index: usize,
    ) -> Result<()> {
        let t = std::time::Instant::now();
        debug!("Recording commands");

        let command_buffer = &mut self.data.command_buffers[image_index];

        command_buffer.begin(&self.data.device)?;

        let render_area = vk::Rect2D::builder()
            .offset(vk::Offset2D::default())
            .extent(self.data.swapchain.as_ref().unwrap().extent);

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
            .render_pass(self.data.pipeline.as_ref().unwrap().render_pass)
            .framebuffer(self.data.framebuffers.as_ref().unwrap()[image_index])
            .render_area(render_area)
            .clear_values(clear_values);

        self.data.device.cmd_begin_render_pass(
            command_buffer.buffer,
            &info,
            vk::SubpassContents::INLINE,
        );
        self.data.device.cmd_bind_pipeline(
            command_buffer.buffer,
            vk::PipelineBindPoint::GRAPHICS,
            self.data.pipeline.as_ref().unwrap().pipeline,
        );

        self.data.device.cmd_bind_descriptor_sets(
            command_buffer.buffer,
            vk::PipelineBindPoint::GRAPHICS,
            self.data.pipeline.as_ref().unwrap().layout,
            0,
            &[self.data.uniforms.as_ref().unwrap().descriptor_sets[image_index]],
            &[],
        );

        let mut to_remove = Vec::new();

        for (i, chunk) in chunks.iter().enumerate() {
            if let Some(chunk) = chunk.upgrade() {
                let chunk = chunk.lock().unwrap();
                self.data.device.cmd_bind_vertex_buffers(
                    command_buffer.buffer,
                    0,
                    &[chunk.vertex_buffer.as_ref().unwrap().buffer],
                    &[0],
                );

                self.data.device.cmd_draw(
                    command_buffer.buffer,
                    chunk.vertices_len as u32,
                    1,
                    0,
                    0,
                );
            } else {
                to_remove.push(i);
            }
        }

        to_remove.reverse();

        for i in to_remove {
            chunks.swap_remove(i);
        }

        self.data.device.cmd_end_render_pass(command_buffer.buffer);

        command_buffer.end(&self.data.device)?;

        debug!("Recording commands took {:?}", t.elapsed());
        Ok(())
    }

    pub unsafe fn update(&mut self, inputs: &Inputs, dt: f32) -> Result<()> {
        self.camera.update(inputs, dt);
        Ok(())
    }

    pub unsafe fn render(
        &mut self,
        window: &Window,
        chunks: &mut Vec<Weak<Mutex<Chunk>>>,
        _dt: f32,
    ) -> Result<()> {
        let t = std::time::Instant::now();
        debug!("Rendering");

        self.data.device.wait_for_fences(
            &[self.data.in_flight_fences[self.frame]],
            true,
            u64::max_value(),
        )?;

        let result = self.data.device.acquire_next_image_khr(
            self.data.swapchain.as_ref().unwrap().swapchain,
            u64::max_value(),
            self.data.image_available_semaphore[self.frame],
            vk::Fence::null(),
        );

        let image_index = match result {
            Ok((image_index, _)) => image_index as usize,
            Err(vk::ErrorCode::OUT_OF_DATE_KHR) => return self.recreate_swapchain(window),
            Err(e) => return Err(anyhow!(e)),
        };

        if !self.data.images_in_flight[image_index as usize].is_null() {
            self.data.device.wait_for_fences(
                &[self.data.images_in_flight[image_index as usize]],
                true,
                u64::max_value(),
            )?;
        }

        self.data.images_in_flight[image_index as usize] = self.data.in_flight_fences[self.frame];

        self.camera.send(&mut self.data, image_index)?;
        self.record_commands(chunks, image_index)?;

        let wait_semaphores = &[self.data.image_available_semaphore[self.frame]];
        let wait_stages = &[vk::PipelineStageFlags::COLOR_ATTACHMENT_OUTPUT];
        let command_buffers = &[self.data.command_buffers[image_index as usize].buffer];
        let signal_semaphores = &[self.data.render_finished_semaphore[self.frame]];
        let submit_info = vk::SubmitInfo::builder()
            .wait_semaphores(wait_semaphores)
            .wait_dst_stage_mask(wait_stages)
            .command_buffers(command_buffers)
            .signal_semaphores(signal_semaphores);

        self.data
            .device
            .reset_fences(&[self.data.in_flight_fences[self.frame]])?;

        self.data.device.queue_submit(
            self.data.graphics_queue,
            &[submit_info],
            self.data.in_flight_fences[self.frame],
        )?;

        let swapchains = &[self.data.swapchain.as_ref().unwrap().swapchain];
        let image_indices = &[image_index as u32];
        let present_info = vk::PresentInfoKHR::builder()
            .wait_semaphores(signal_semaphores)
            .swapchains(swapchains)
            .image_indices(image_indices);

        let result = self
            .data
            .device
            .queue_present_khr(self.data.present_queue, &present_info);

        let changed = result == Ok(vk::SuccessCode::SUBOPTIMAL_KHR)
            || result == Err(vk::ErrorCode::OUT_OF_DATE_KHR);

        if changed || self.resized {
            self.recreate_swapchain(window)?;
            self.resized = false;
        } else if let Err(e) = result {
            return Err(anyhow!(e));
        }

        self.frame = (self.frame + 1) % MAX_FRAMES_IN_FLIGHT;

        debug!("Rendering took {:?}", t.elapsed());

        Ok(())
    }

    pub unsafe fn recreate_swapchain(&mut self, window: &Window) -> Result<()> {
        self.data.device.device_wait_idle()?;

        println!("Recreating swapchain");

        self.data.uniforms = None;
        self.data.depth_buffer = None;
        self.data.framebuffers = None;
        self.data.device.free_command_buffers(
            self.data.command_pool.as_ref().unwrap().pool,
            &self
                .data
                .command_buffers
                .iter()
                .map(|b| b.buffer)
                .collect::<Vec<vk::CommandBuffer>>(),
        );
        self.data.command_buffers.clear();
        self.data.pipeline = None;
        self.data.swapchain = None;

        self.data.swapchain = Some(Swapchain::create(window, &self.data)?);
        self.data.uniforms = Some(Uniforms::create(&mut self.data)?);
        self.data.depth_buffer = Some(DepthBuffer::create(&mut self.data)?);
        self.data.pipeline = Some(Pipeline::create(&self.data)?);
        self.data.framebuffers = Some(Framebuffers::create(&self.data)?);
        self.data.command_buffers = self
            .data
            .command_pool
            .as_mut()
            .unwrap()
            .allocate_command_buffers(
                &self.data.device,
                self.data.swapchain.as_ref().unwrap().images.len() as u32,
            )?;
        self.data.images_in_flight.resize(
            self.data.swapchain.as_ref().unwrap().images.len(),
            vk::Fence::null(),
        );
        self.camera.update_projection(&self.data);
        self.camera.send_all(&mut self.data)?;

        Ok(())
    }
}

impl Drop for Renderer {
    fn drop(&mut self) {
        unsafe {
            self.data.device.device_wait_idle().unwrap();

            // set all options to None to call Drop in the right order
            self.data.depth_buffer = None;
            self.data.uniforms = None;
            self.data.framebuffers = None;
            self.data.command_buffers.clear();
            self.data.command_pool = None;
            self.data.pipeline = None;
            self.data.swapchain = None;

            self.destroy_sync_objects().unwrap();

            device::destroy(&mut self.data.device);
            self.data
                .instance
                .destroy_surface_khr(self.data.surface, None);
            instance::destroy(&mut self.data);
        }
    }
}

pub struct RendererData {
    pub instance: Instance,
    pub messenger: Option<vk::DebugUtilsMessengerEXT>,
    pub surface: vk::SurfaceKHR,
    pub physical_device: vk::PhysicalDevice,
    pub device: Arc<Device>,
    pub graphics_queue: vk::Queue,
    pub present_queue: vk::Queue,
    pub allocator: Arc<Allocator>,
    pub swapchain: Option<Swapchain>,
    pub pipeline: Option<Pipeline>,
    pub framebuffers: Option<Framebuffers>,
    pub command_pool: Option<CommandPool>,
    pub command_buffers: Vec<CommandBuffer>,
    pub image_available_semaphore: Vec<vk::Semaphore>,
    pub render_finished_semaphore: Vec<vk::Semaphore>,
    pub in_flight_fences: Vec<vk::Fence>,
    pub images_in_flight: Vec<vk::Fence>,
    pub uniforms: Option<Uniforms<UniformBufferObject>>,
    pub depth_buffer: Option<DepthBuffer>,
}

impl RendererData {
    pub fn new(
        instance: Instance,
        messenger: Option<vk::DebugUtilsMessengerEXT>,
        surface: vk::SurfaceKHR,
        physical_device: vk::PhysicalDevice,
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
            images_in_flight: Vec::new(),
            uniforms: None,
            depth_buffer: None,
        }
    }
}
