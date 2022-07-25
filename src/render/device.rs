use anyhow::Result;
use std::collections::HashMap;
use std::sync::Arc;
use vulkanalia::vk::{DeviceV1_0, HasBuilder};
use vulkanalia::{vk, Device, Instance};

use crate::config::{DEVICE_EXTENSIONS, VALIDATION_ENABLED, VALIDATION_LAYER};
use crate::render::physical_device::QueueDef;

use super::physical_device::PhysicalDevice;

pub unsafe fn create(
    instance: &Instance,
    physical_device: &PhysicalDevice,
) -> Result<(vulkanalia::Device, vk::Queue, vk::Queue)> {
    let mut queues = HashMap::new();
    let mut max_family = 0;
    let mut insert = |queue: &QueueDef| {
        if queue.family > max_family {
            max_family = queue.family;
        }
        if let Some(value) = queues.get_mut(&queue.family) {
            if queue.index > *value {
                *value = queue.index;
            }
        } else {
            queues.insert(queue.family, queue.index);
        }
    };
    insert(&physical_device.graphics_queue);
    insert(&physical_device.present_queue);

    for queue in &physical_device.transfer_queues {
        insert(queue);
    }

    let mut priorities = vec![Vec::new(); max_family as usize + 1];
    for i in 0..=max_family {
        if let Some(queue_count) = queues.get(&i) {
            // +1 because we store the max index and vulkan want the count
            priorities[i as usize].resize(*queue_count as usize + 1, 1.0);
        }
    }

    let queue_infos: Vec<_> = queues
        .iter()
        .map(|i| {
            vk::DeviceQueueCreateInfo::builder()
                .queue_family_index(*i.0)
                .queue_priorities(&priorities[*i.0 as usize])
        })
        .collect();

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

    let device = instance.create_device(physical_device.device, &info, None)?;

    let graphics_queue = device.get_device_queue(
        physical_device.graphics_queue.family,
        physical_device.graphics_queue.index,
    );
    let present_queue = device.get_device_queue(
        physical_device.present_queue.family,
        physical_device.present_queue.index,
    );

    Ok((device, graphics_queue, present_queue))
}

#[inline]
pub unsafe fn destroy(device: &mut Arc<Device>) {
    device.destroy_device(None);
}
