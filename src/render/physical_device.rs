use std::collections::HashSet;

use anyhow::{anyhow, Result};
use vulkanalia::{
    vk::{self, InstanceV1_0},
    Instance,
};

use log::*;

use crate::config::DEVICE_EXTENSIONS;

use super::queue::QueueFamilyIndices;
use super::swapchain::SwapchainSupport;

pub fn pick(instance: &Instance, surface: vk::SurfaceKHR) -> Result<vk::PhysicalDevice> {
    for physical_device in unsafe { instance.enumerate_physical_devices()? } {
        let properties = unsafe { instance.get_physical_device_properties(physical_device) };

        if let Err(error) = unsafe { check_physical_device(instance, surface, physical_device) } {
            warn!(
                "Skipping physical device (`{}`): {}",
                properties.device_name, error
            );
        } else {
            info!("Selected physical device (`{}`).", properties.device_name);
            return Ok(physical_device);
        }
    }
    Err(anyhow!("Failed to find suitable physical device."))
}

unsafe fn check_physical_device(
    instance: &Instance,
    surface: vk::SurfaceKHR,
    physical_device: vk::PhysicalDevice,
) -> Result<()> {
    QueueFamilyIndices::get(instance, surface, physical_device)?;
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

    Ok(())
}
