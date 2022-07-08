use anyhow::Result;
use std::collections::HashSet;
use std::sync::Arc;
use vulkanalia::vk::{DeviceV1_0, HasBuilder};
use vulkanalia::{vk, Instance, Device};

use crate::config::{DEVICE_EXTENSIONS, VALIDATION_ENABLED, VALIDATION_LAYER};

use super::queue::QueueFamilyIndices;

pub unsafe fn create(instance: &Instance, surface: vk::SurfaceKHR, physical_device: vk::PhysicalDevice) -> Result<(vulkanalia::Device, vk::Queue, vk::Queue)> {
    let indices = QueueFamilyIndices::get(instance, surface, physical_device)?;

    let mut unique_indices = HashSet::new();
    unique_indices.insert(indices.graphics);
    unique_indices.insert(indices.present);

    let queue_priorities = &[1.0];
    let queue_infos = unique_indices
        .iter()
        .map(|i| {
            vk::DeviceQueueCreateInfo::builder()
                .queue_family_index(*i)
                .queue_priorities(queue_priorities)
        })
        .collect::<Vec<_>>();

    let layers = if VALIDATION_ENABLED {
        vec![VALIDATION_LAYER.as_ptr()]
    } else {
        Vec::new()
    };

    let features = vk::PhysicalDeviceFeatures::builder().sampler_anisotropy(true);

    let extensions = DEVICE_EXTENSIONS
        .iter()
        .map(|n| n.as_ptr())
        .collect::<Vec<_>>();

    let info = vk::DeviceCreateInfo::builder()
        .queue_create_infos(&queue_infos)
        .enabled_layer_names(&layers)
        .enabled_features(&features)
        .enabled_extension_names(&extensions);

    let device = instance.create_device(physical_device, &info, None)?;

    let graphics_queue = device.get_device_queue(indices.graphics, 0);
    let present_queue = device.get_device_queue(indices.present, 0);

    Ok((device, graphics_queue, present_queue))
}

#[inline]
pub unsafe fn destroy(device: &mut Arc<Device>) {
    device.destroy_device(None);
}
