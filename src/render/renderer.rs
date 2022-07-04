use anyhow::{anyhow, Result};
use vulkanalia::{
    self,
    vk::{self, DeviceV1_0, Handle, HasBuilder, KhrSurfaceExtension, KhrSwapchainExtension},
    Device, Entry, Instance,
};
use winit::window::Window;

use crate::config::MAX_FRAMES_IN_FLIGHT;

use super::{
    commands::{CommandBuffer, CommandPool},
    device,
    framebuffers::Framebuffers,
    instance, physical_device,
    pipeline::Pipeline,
    swapchain::Swapchain,
    sync,
};

pub struct Renderer {
    instance: Instance,
    device: vulkanalia::Device,
    data: RendererData,
    frame: usize,
    pub resized: bool,
}

impl Renderer {
    pub unsafe fn new(window: &Window, entry: &Entry) -> Result<Self> {
        let mut data = RendererData::default();
        let instance = instance::create(window, entry, &mut data)?;
        data.surface = vulkanalia::window::create_surface(&instance, window)?;

        physical_device::pick(&instance, &mut data)?;

        let device = device::create(&instance, &mut data)?;

        data.swapchain = Swapchain::create(window, &instance, &device, &mut data)?;
        data.pipeline = Pipeline::create(&device, &data)?;
        data.framebuffers = Framebuffers::create(&device, &data)?;
        data.command_pool = CommandPool::create(&instance, &device, &data)?;
        data.command_buffers = data
            .command_pool
            .allocate_command_buffers(&device, data.swapchain.images.len() as u32)?;

        create_sync_objects(&device, &mut data)?;

        Ok(Self {
            instance,
            device,
            data,
            frame: 0,
            resized: false,
        })
    }

    pub unsafe fn record_commands(&mut self) -> Result<()> {
        for (i, command_buffer) in self.data.command_buffers.iter_mut().enumerate() {
            command_buffer.begin(&self.device)?;

            let render_area = vk::Rect2D::builder()
                .offset(vk::Offset2D::default())
                .extent(self.data.swapchain.extent);

            let color_clear_value = vk::ClearValue {
                color: vk::ClearColorValue {
                    float32: [0.0, 0.0, 0.0, 1.0],
                },
            };

            let clear_values = &[color_clear_value];
            let info = vk::RenderPassBeginInfo::builder()
                .render_pass(self.data.pipeline.render_pass)
                .framebuffer(self.data.framebuffers[i])
                .render_area(render_area)
                .clear_values(clear_values);

            self.device.cmd_begin_render_pass(
                command_buffer.buffer,
                &info,
                vk::SubpassContents::INLINE,
            );
            self.device.cmd_bind_pipeline(
                command_buffer.buffer,
                vk::PipelineBindPoint::GRAPHICS,
                self.data.pipeline.pipeline,
            );
            self.device.cmd_draw(command_buffer.buffer, 3, 1, 0, 0);
            self.device.cmd_end_render_pass(command_buffer.buffer);

            command_buffer.end(&self.device)?;
        }
        Ok(())
    }

    pub unsafe fn render(&mut self, window: &Window) -> Result<()> {
        self.device.wait_for_fences(
            &[self.data.in_flight_fences[self.frame]],
            true,
            u64::max_value(),
        )?;

        let result = self.device.acquire_next_image_khr(
            self.data.swapchain.swapchain,
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
            self.device.wait_for_fences(
                &[self.data.images_in_flight[image_index as usize]],
                true,
                u64::max_value(),
            )?;
        }

        self.data.images_in_flight[image_index as usize] = self.data.in_flight_fences[self.frame];

        let wait_semaphores = &[self.data.image_available_semaphore[self.frame]];
        let wait_stages = &[vk::PipelineStageFlags::COLOR_ATTACHMENT_OUTPUT];
        let command_buffers = &[self.data.command_buffers[image_index as usize].buffer];
        let signal_semaphores = &[self.data.render_finished_semaphore[self.frame]];
        let submit_info = vk::SubmitInfo::builder()
            .wait_semaphores(wait_semaphores)
            .wait_dst_stage_mask(wait_stages)
            .command_buffers(command_buffers)
            .signal_semaphores(signal_semaphores);

        self.device
            .reset_fences(&[self.data.in_flight_fences[self.frame]])?;

        self.device.queue_submit(
            self.data.graphics_queue,
            &[submit_info],
            self.data.in_flight_fences[self.frame],
        )?;

        let swapchains = &[self.data.swapchain.swapchain];
        let image_indices = &[image_index as u32];
        let present_info = vk::PresentInfoKHR::builder()
            .wait_semaphores(signal_semaphores)
            .swapchains(swapchains)
            .image_indices(image_indices);

        let result = self
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

        Ok(())
    }

    pub unsafe fn destroy_swapchain(&mut self) -> Result<()> {
        self.data.framebuffers.destroy(&self.device);
        self.device.free_command_buffers(
            self.data.command_pool.pool,
            &self
                .data
                .command_buffers
                .iter()
                .map(|b| b.buffer)
                .collect::<Vec<vk::CommandBuffer>>(),
        );
        self.data.pipeline.destroy(&self.device);
        self.data.swapchain.destroy(&self.device);
        Ok(())
    }

    pub unsafe fn recreate_swapchain(&mut self, window: &Window) -> Result<()> {
        self.device.device_wait_idle()?;
        self.destroy_swapchain()?;

        self.data.swapchain =
            Swapchain::create(window, &self.instance, &self.device, &mut self.data)?;
        self.data.pipeline = Pipeline::create(&self.device, &self.data)?;
        self.data.framebuffers = Framebuffers::create(&self.device, &self.data)?;
        self.data.command_buffers = self
            .data
            .command_pool
            .allocate_command_buffers(&self.device, self.data.swapchain.images.len() as u32)?;
        self.data
            .images_in_flight
            .resize(self.data.swapchain.images.len(), vk::Fence::null());
        self.record_commands()?;
        Ok(())
    }
}

impl Drop for Renderer {
    fn drop(&mut self) {
        unsafe {
            self.device.device_wait_idle().unwrap();
            self.destroy_swapchain().unwrap();

            self.data
                .image_available_semaphore
                .iter()
                .for_each(|s| self.device.destroy_semaphore(*s, None));
            self.data
                .render_finished_semaphore
                .iter()
                .for_each(|s| self.device.destroy_semaphore(*s, None));
            self.data
                .in_flight_fences
                .iter()
                .for_each(|f| self.device.destroy_fence(*f, None));

            self.data.command_pool.destroy(&self.device);
            device::destroy(&mut self.device);
            self.instance.destroy_surface_khr(self.data.surface, None);
            instance::destroy(&mut self.instance, &mut self.data);
        }
    }
}

#[derive(Default)]
pub struct RendererData {
    pub messenger: vk::DebugUtilsMessengerEXT,
    pub physical_device: vk::PhysicalDevice,
    pub surface: vk::SurfaceKHR,
    pub graphics_queue: vk::Queue,
    pub present_queue: vk::Queue,
    pub swapchain: Swapchain,
    pub pipeline: Pipeline,
    pub framebuffers: Framebuffers,
    pub command_pool: CommandPool,
    pub command_buffers: Vec<CommandBuffer>,
    pub image_available_semaphore: Vec<vk::Semaphore>,
    pub render_finished_semaphore: Vec<vk::Semaphore>,
    pub in_flight_fences: Vec<vk::Fence>,
    pub images_in_flight: Vec<vk::Fence>,
}

unsafe fn create_sync_objects(device: &Device, data: &mut RendererData) -> Result<()> {
    data.image_available_semaphore = sync::create_semaphores(device, MAX_FRAMES_IN_FLIGHT)?;
    data.render_finished_semaphore = sync::create_semaphores(device, MAX_FRAMES_IN_FLIGHT)?;
    data.in_flight_fences = sync::create_fences(device, true, MAX_FRAMES_IN_FLIGHT)?;
    data.images_in_flight = data
        .swapchain
        .images
        .iter()
        .map(|_| vk::Fence::null())
        .collect();
    Ok(())
}
