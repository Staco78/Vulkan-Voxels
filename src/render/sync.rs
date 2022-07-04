use anyhow::Result;
use vulkanalia::{
    vk::{self, DeviceV1_0, HasBuilder},
    Device,
};

// #[inline]
// pub unsafe fn create_semaphore(device: &Device) -> Result<vk::Semaphore> {
//     let info = vk::SemaphoreCreateInfo::builder();
//     let semaphore = device.create_semaphore(&info, None)?;
//     Ok(semaphore)
// }

// #[inline]
// pub unsafe fn create_fence(device: &Device, signaled: bool) -> Result<vk::Fence> {
//     let info = vk::FenceCreateInfo::builder().flags(if signaled {
//         vk::FenceCreateFlags::SIGNALED
//     } else {
//         vk::FenceCreateFlags::empty()
//     });
//     let fence = device.create_fence(&info, None)?;
//     Ok(fence)
// }

#[inline]
pub unsafe fn create_semaphores(device: &Device, count: usize) -> Result<Vec<vk::Semaphore>> {
    let info = vk::SemaphoreCreateInfo::builder();
    let mut semaphores = Vec::with_capacity(count);
    for _ in 0..count {
        semaphores.push(device.create_semaphore(&info, None)?);
    }
    Ok(semaphores)
}

#[inline]
pub unsafe fn create_fences(
    device: &Device,
    signaled: bool,
    count: usize,
) -> Result<Vec<vk::Fence>> {
    let info = vk::FenceCreateInfo::builder().flags(if signaled {
        vk::FenceCreateFlags::SIGNALED
    } else {
        vk::FenceCreateFlags::empty()
    });
    let mut fences = Vec::with_capacity(count);
    for _ in 0..count {
        fences.push(device.create_fence(&info, None)?);
    }
    Ok(fences)
}
