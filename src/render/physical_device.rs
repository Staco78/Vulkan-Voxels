use std::collections::HashSet;

use anyhow::{anyhow, Result};
use vulkanalia::{
    vk::{self, InstanceV1_0, KhrSurfaceExtension, QueueFlags},
    Instance,
};

use log::*;

use crate::{config::DEVICE_EXTENSIONS};

use super::swapchain::SwapchainSupport;

#[derive(Debug, PartialEq, Hash, Clone, Copy, Eq)]
pub struct QueueDef {
    pub family: u32,
    pub index: u32,
}

impl QueueDef {
    pub fn new(family: u32, index: u32) -> Self {
        Self { family, index }
    }
}

pub struct PhysicalDevice {
    pub device: vk::PhysicalDevice,
    pub graphics_queue: QueueDef,
    pub present_queue: QueueDef,
    pub transfer_queues: Vec<QueueDef>,
}

impl PhysicalDevice {
    pub fn pick(instance: &Instance, surface: vk::SurfaceKHR) -> Result<Self> {
        for physical_device in unsafe { instance.enumerate_physical_devices()? } {
            let properties = unsafe { instance.get_physical_device_properties(physical_device) };

            match unsafe { check_physical_device(instance, surface, physical_device) } {
                Ok(device) => {
                    info!("Selected physical device (`{}`).", properties.device_name);
                    return Ok(device);
                }
                Err(e) => {
                    warn!(
                        "Skipping physical device (`{}`): {}",
                        properties.device_name, e
                    );
                }
            }
        }
        Err(anyhow!("Failed to find suitable physical device."))
    }
}

unsafe fn check_physical_device(
    instance: &Instance,
    surface: vk::SurfaceKHR,
    physical_device: vk::PhysicalDevice,
) -> Result<PhysicalDevice> {
    let queues = get_queues(instance, surface, physical_device)?;
    {
        let extensions = instance
            .enumerate_device_extension_properties(physical_device, None)?
            .iter()
            .map(|e| e.extension_name)
            .collect::<HashSet<_>>();
        if DEVICE_EXTENSIONS.iter().all(|e| extensions.contains(e)) {
            Ok(())
        } else {
            Err(anyhow!("Missing required device extensions."))
        }
    }?;

    let support = SwapchainSupport::get(instance, surface, physical_device)?;
    if support.formats.is_empty() || support.present_modes.is_empty() {
        return Err(anyhow!("Insufficient swapchain support."));
    }

    let device = PhysicalDevice {
        device: physical_device,
        graphics_queue: queues.0,
        present_queue: queues.1,
        transfer_queues: queues.2,
    };

    Ok(device)
}

pub unsafe fn get_queues(
    instance: &Instance,
    surface: vk::SurfaceKHR,
    physical_device: vk::PhysicalDevice,
) -> Result<(QueueDef, QueueDef, Vec<QueueDef>)> {
    let properties = instance.get_physical_device_queue_family_properties(physical_device);

    let mut graphics = None;
    let mut present = None;
    let mut transfer = Vec::new();

    for (index, family) in properties.iter().enumerate() {
        assert!(family.queue_count > 0);
        let mut min_queue_index = 0;
        if graphics.is_none() && family.queue_flags.contains(QueueFlags::GRAPHICS) {
            // if graphics family: take first queue
            graphics = Some(QueueDef::new(index as u32, 0));
            min_queue_index = 1;
        }
        if present.is_none()
            && instance.get_physical_device_surface_support_khr(
                physical_device,
                index as u32,
                surface,
            )?
        {
            present = Some(QueueDef::new(index as u32, 0));
            min_queue_index = 1;
        }

        if family.queue_flags.contains(QueueFlags::TRANSFER) {
            for i in min_queue_index..family.queue_count {
                transfer.push(QueueDef::new(index as u32, i));
            }
        }
    }

    if transfer.len() == 0 {
        Err(anyhow!("No transfer queue found"))
    } else {
        let r = (
            graphics.expect("No graphics queue found"),
            present.expect("No present queue family found"),
            transfer,
        );
        Ok(r)
    }
}
