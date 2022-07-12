use anyhow::anyhow;
use anyhow::Result;
use vulkanalia::vk::KhrSurfaceExtension;
use vulkanalia::{
    vk::{self, InstanceV1_0},
    Instance,
};

use crate::threads::MESHING_THREADS_COUNT;

pub struct QueueFamilyIndices {
    pub graphics: u32,
    pub present: u32,
    pub transfer: u32,
}

impl QueueFamilyIndices {
    pub unsafe fn get(
        instance: &Instance,
        surface: vk::SurfaceKHR,
        physical_device: vk::PhysicalDevice,
    ) -> Result<Self> {
        let properties = instance.get_physical_device_queue_family_properties(physical_device);

        let graphics = properties
            .iter()
            .position(|p| p.queue_flags.contains(vk::QueueFlags::GRAPHICS))
            .map(|i| i as u32)
            .ok_or_else(|| anyhow!("No graphics queue family found."))?;

        let transfer = properties
            .iter()
            .position(|p| {
                p.queue_flags.contains(vk::QueueFlags::TRANSFER)
                    && p != &properties[graphics as usize]
                    && p.queue_count >= MESHING_THREADS_COUNT as u32
            })
            .map(|i| i as u32)
            .ok_or_else(|| anyhow!("No valid transfert queue family found: required queue count: {MESHING_THREADS_COUNT} found: {}.", properties.iter().fold(0, |acc, p| {
                if p.queue_flags.contains(vk::QueueFlags::TRANSFER)
                && p != &properties[graphics as usize] {
                    if acc < p.queue_count {
                       return p.queue_count
                    }
                }
                acc
            })))?;

        let mut present = None;
        for (index, _properties) in properties.iter().enumerate() {
            if instance.get_physical_device_surface_support_khr(
                physical_device,
                index as u32,
                surface,
            )? && index as u32 != transfer
            {
                present = Some(index as u32);
                break;
            }
        }

        if let Some(present) = present {
            Ok(Self {
                graphics,
                present,
                transfer,
            })
        } else {
            Err(anyhow!("No present queue family found."))
        }
    }
}
