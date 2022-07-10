use std::sync::{Arc, Weak};

use anyhow::{anyhow, Result};
use vulkanalia::{
    vk::{self, DeviceV1_0, HasBuilder, InstanceV1_0},
    Device, Instance,
};

#[derive(Copy, Clone, Debug)]
pub enum AllocUsage {
    Staging,
    DeviceLocal,
}

#[derive(Copy, Clone, Debug)]
pub struct AllocRequirements {
    pub size: u64,
    pub alignment: u64,
    pub usage: AllocUsage,
    pub memory_type_bits: u32,
}

impl AllocRequirements {
    pub fn new(requirements: vk::MemoryRequirements, usage: AllocUsage) -> Self {
        Self {
            size: requirements.size,
            alignment: requirements.alignment,
            usage,
            memory_type_bits: requirements.memory_type_bits,
        }
    }
}

pub struct Allocator {
    device: Weak<Device>,
    memory_properties: vk::PhysicalDeviceMemoryProperties,
}

impl Allocator {
    pub unsafe fn new(
        device: &Arc<Device>,
        instance: &Instance,
        physical_device: vk::PhysicalDevice,
    ) -> Self {
        Self {
            device: Arc::downgrade(device),
            memory_properties: instance.get_physical_device_memory_properties(physical_device),
        }
    }

    fn get_memory_properties(&self, requirements: AllocRequirements) -> vk::MemoryPropertyFlags {
        match requirements.usage {
            AllocUsage::Staging => vk::MemoryPropertyFlags::HOST_VISIBLE,
            AllocUsage::DeviceLocal => vk::MemoryPropertyFlags::DEVICE_LOCAL,
        }
    }

    pub unsafe fn get_memory_type_index(
        &self,
        properties: vk::MemoryPropertyFlags,
        requirements: vk::MemoryRequirements,
    ) -> Result<u32> {
        (0..self.memory_properties.memory_type_count)
            .find(|i| {
                let suitable = (requirements.memory_type_bits & (1 << i)) != 0;
                let memory_type = self.memory_properties.memory_types[*i as usize];
                suitable && memory_type.property_flags.contains(properties)
            })
            .ok_or_else(|| anyhow!("Failed to find suitable memory type."))
    }

    pub unsafe fn alloc(&self, requirements: AllocRequirements) -> Result<vk::DeviceMemory> {
        let properties = self.get_memory_properties(requirements);
        let memory_type_index = self.get_memory_type_index(
            properties,
            vk::MemoryRequirements {
                size: requirements.size,
                alignment: requirements.alignment,
                memory_type_bits: requirements.memory_type_bits,
            },
        )?;

        let memory_info = vk::MemoryAllocateInfo::builder()
            .allocation_size(requirements.size)
            .memory_type_index(memory_type_index);

        let memory = self
            .device
            .upgrade()
            .unwrap()
            .allocate_memory(&memory_info, None)?;

        Ok(memory)
    }

    pub unsafe fn free(&self, memory: vk::DeviceMemory) {
        self.device
            .upgrade()
            .unwrap()
            .free_memory(memory, None);
    }
}
