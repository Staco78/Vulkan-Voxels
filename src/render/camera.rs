use std::mem::size_of;

use anyhow::Result;
use glm::{vec3, Mat4, Vec3};
use nalgebra_glm as glm;
use vulkanalia::Device;

use crate::inputs::Inputs;

use super::renderer::{RendererData, UniformBufferObject};

pub struct Camera {
    model: glm::Mat4,
    view: glm::Mat4,
    proj: glm::Mat4,

    pub pos: Vec3,
    fov: f32,
    near: f32,
    far: f32,

    yaw: f32,
    pitch: f32,
}

impl Camera {
    pub unsafe fn new(device: &Device, data: &mut RendererData) -> Result<Camera> {
        let mut cam = Camera {
            model: glm::identity(),
            view: Mat4::default(),
            proj: Mat4::default(),
            pos: vec3(-20.0, 0.0, 0.0),
            fov: 45.0,
            near: 0.1,
            far: 1000.0,
            yaw: 0.0,
            pitch: 0.0,
        };

        cam.update_view();
        cam.update_projection(data);

        cam.send_all(device, data)?;

        Ok(cam)
    }

    pub unsafe fn send_all(&self, device: &Device, data: &mut RendererData) -> Result<()> {
        let ubo = UniformBufferObject {
            model: self.model,
            view: self.view,
            proj: self.proj,
        };

        data.uniforms.buffers.iter_mut().for_each(|b| {
            b.fill(device, &ubo, 1).unwrap();
        });

        Ok(())
    }

    pub unsafe fn send(
        &self,
        device: &Device,
        data: &mut RendererData,
        image_index: usize,
    ) -> Result<()> {
        let ptr = data.uniforms.buffers[image_index].map(
            device,
            size_of::<glm::Mat4>() as u64,
            size_of::<glm::Mat4>() as u64,
        )?;
        *ptr = self.view;
        data.uniforms.buffers[image_index].unmap(device)?;

        Ok(())
    }

    pub unsafe fn update(&mut self, inputs: &Inputs, dt: f32) {
        const SENSITIVITY: f32 = 5.0;

        self.yaw += inputs.mouse_delta.0 as f32 * dt * SENSITIVITY;
        self.pitch -= inputs.mouse_delta.1 as f32 * dt * SENSITIVITY;

        if self.pitch > 89.0 {
            self.pitch = 89.0;
        }
        if self.pitch < -89.0 {
            self.pitch = -89.0;
        }

        let dir =
            Vec3::new(self.yaw.to_radians().cos(), 0., self.yaw.to_radians().sin()).normalize();
        let right = dir.cross(&Vec3::y()).normalize();
        let up = Vec3::y();

        let speed = 5. * dt;

        if inputs.is_key_pressed(winit::event::VirtualKeyCode::Z) {
            self.pos += dir * speed;
        }
        if inputs.is_key_pressed(winit::event::VirtualKeyCode::S) {
            self.pos -= dir * speed;
        }
        if inputs.is_key_pressed(winit::event::VirtualKeyCode::Q) {
            self.pos -= right * speed;
        }
        if inputs.is_key_pressed(winit::event::VirtualKeyCode::D) {
            self.pos += right * speed;
        }
        if inputs.is_key_pressed(winit::event::VirtualKeyCode::Space) {
            self.pos += up * speed;
        }
        if inputs.is_key_pressed(winit::event::VirtualKeyCode::LShift) {
            self.pos -= up * speed;
        }

        self.update_view();
    }

    fn update_view(&mut self) {
        let mut front = Vec3::default();
        front.x = self.yaw.to_radians().cos() * self.pitch.to_radians().cos();
        front.y = self.pitch.to_radians().sin();
        front.z = self.yaw.to_radians().sin() * self.pitch.to_radians().cos();
        let rotation = front.normalize();

        self.view = glm::look_at(&self.pos, &(self.pos + rotation), &glm::vec3(0.0, 1.0, 0.0));
    }

    pub fn update_projection(&mut self, data: &RendererData) {
        self.proj = glm::perspective_rh_zo(
            data.swapchain.extent.width as f32 / data.swapchain.extent.height as f32,
            self.fov.to_radians(),
            self.near,
            self.far,
        );
        self.proj[(1, 1)] *= -1.0;
    }
}
