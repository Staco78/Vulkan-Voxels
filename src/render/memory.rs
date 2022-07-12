use std::{
    ptr,
    sync::{
        atomic::{AtomicU64, Ordering},
        Arc, RwLock, Weak,
    },
};

use anyhow::{anyhow, Result};
use log::trace;
use vulkanalia::{
    vk::{self, DeviceV1_0, HasBuilder, InstanceV1_0},
    Device, Instance,
};

#[derive(Copy, Clone, Debug, PartialEq)]
pub enum AllocUsage {
    Staging,
    DeviceLocal,
}

const MIN_ALLOC_SIZE: usize = 1024 * 1024;

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

#[derive(Debug)]
pub struct Allocator {
    // device: Weak<Device>,
    memory_properties: vk::PhysicalDeviceMemoryProperties,
    pools: Vec<Pool>,
}

impl Allocator {
    pub unsafe fn new(
        device: &Arc<Device>,
        instance: &Instance,
        physical_device: vk::PhysicalDevice,
    ) -> Self {
        let memory_properties = instance.get_physical_device_memory_properties(physical_device);
        let mut pools = Vec::with_capacity(memory_properties.memory_type_count as usize);
        for i in 0..memory_properties.memory_type_count {
            pools.push(Pool::new(device, i as u32));
        }
        Self {
            // device: Arc::downgrade(device),
            memory_properties,
            pools,
        }
    }

    fn get_memory_properties(
        _memory_properties: vk::PhysicalDeviceMemoryProperties,
        usage: AllocUsage,
    ) -> vk::MemoryPropertyFlags {
        match usage {
            AllocUsage::Staging => vk::MemoryPropertyFlags::HOST_VISIBLE | vk::MemoryPropertyFlags::HOST_COHERENT,
            AllocUsage::DeviceLocal => vk::MemoryPropertyFlags::DEVICE_LOCAL,
        }
    }

    pub fn get_memory_type_index(
        memory_properties: vk::PhysicalDeviceMemoryProperties,
        properties: vk::MemoryPropertyFlags,
        requirements: vk::MemoryRequirements,
    ) -> Result<u32> {
        (0..memory_properties.memory_type_count)
            .find(|i| {
                let suitable = (requirements.memory_type_bits & (1 << i)) != 0;
                let memory_type = memory_properties.memory_types[*i as usize];
                suitable && memory_type.property_flags.contains(properties)
            })
            .ok_or_else(|| anyhow!("Failed to find suitable memory type."))
    }

    pub unsafe fn alloc(&self, requirements: AllocRequirements) -> Result<(Block, *mut u8)> {
        let properties =
            Allocator::get_memory_properties(self.memory_properties, requirements.usage);
        let memory_type_index = Allocator::get_memory_type_index(
            self.memory_properties,
            properties,
            vk::MemoryRequirements {
                size: requirements.size,
                alignment: requirements.alignment,
                memory_type_bits: requirements.memory_type_bits,
            },
        )?;

        let pool = &self.pools[memory_type_index as usize];
        pool.alloc(requirements.size, requirements.alignment, requirements.usage == AllocUsage::Staging)
    }

    pub unsafe fn free(&self, block: Block) {
        let pool = &self.pools[block.memory_type_index as usize];
        pool.free(block);
    }

    pub unsafe fn free_all(&mut self) {
        self.pools.clear();
    }
}

#[derive(Debug)]
struct Pool {
    device: Weak<Device>,
    memory_type_index: u32,
    chunks: RwLock<Vec<Chunk>>,
    size: AtomicU64,
}

impl Pool {
    fn new(device: &Arc<Device>, memory_type_index: u32) -> Self {
        trace!("Creating memory pool for memory type {}", memory_type_index);
        Self {
            device: Arc::downgrade(device),
            memory_type_index,
            chunks: RwLock::new(Vec::new()),
            size: AtomicU64::new(MIN_ALLOC_SIZE as u64),
        }
    }

    unsafe fn alloc(&self, size: u64, alignment: u64, map: bool) -> Result<(Block, *mut u8)> {
        trace!("Allocating {} bytes from memory pool", size);
        for chunk in self.chunks.read().unwrap().iter() {
            if let Some(block) = chunk.alloc(size, alignment, self.memory_type_index) {
                return Ok(block);
            }
        }

        // no chunk has enough space, create a new one

        let mut new_size = size.max(self.size.load(Ordering::Relaxed));
        // round up size to next power of 2
        new_size -= 1;
        new_size |= new_size >> 1;
        new_size |= new_size >> 2;
        new_size |= new_size >> 4;
        new_size |= new_size >> 8;
        new_size |= new_size >> 16;
        new_size |= new_size >> 32;
        new_size += 1;

        self.size.store(new_size, Ordering::Relaxed);

        let chunk = Chunk::new(
            &self.device.upgrade().unwrap(),
            new_size,
            self.memory_type_index,
            map
        )?;
        let block = chunk
            .alloc(size, alignment, self.memory_type_index)
            .expect("New chunk should have space.");

        self.chunks.write().unwrap().push(chunk);

        Ok(block)
    }

    unsafe fn free(&self, block: Block) {
        let chunks = self.chunks.read().unwrap();
        let chunk = chunks
            .iter()
            .find(|chunk| chunk.memory == block.memory)
            .unwrap();
        chunk.free(block);
    }
}

impl Drop for Pool {
    fn drop(&mut self) {
        trace!("Dropping memory pool");
        let chunks = self.chunks.write().unwrap();
        for chunk in chunks.iter() {
            unsafe {
                if !chunk.ptr.is_null() {
                    self.device.upgrade().unwrap().unmap_memory(chunk.memory);
                }
                self.device
                    .upgrade()
                    .unwrap()
                    .free_memory(chunk.memory, None);
            }
        }
    }
}

#[derive(Debug)]
struct Chunk {
    memory: vk::DeviceMemory,
    blocks: RwLock<Vec<Block>>,
    size: u64,
    ptr: *mut u8,
}

unsafe impl Send for Chunk {}
unsafe impl Sync for Chunk {}

impl Chunk {
    unsafe fn new(device: &Device, size: u64, memory_type_index: u32, map: bool) -> Result<Self> {
        trace!(
            "Creating chunk of {} bytes and memory type {}",
            size,
            memory_type_index
        );
        let info = vk::MemoryAllocateInfo::builder()
            .allocation_size(size)
            .memory_type_index(memory_type_index);
        let memory = device.allocate_memory(&info, None)?;
        let block = Block::new(memory, memory_type_index, 0, size);

        let ptr = if map {
            device.map_memory(
                memory,
                0,
                vk::WHOLE_SIZE as u64,
                vk::MemoryMapFlags::empty(),
            )?.cast()
        } else {
            ptr::null_mut()
        };

        Ok(Self {
            memory,
            blocks: RwLock::new(vec![block]),
            size,
            ptr,
        })
    }

    unsafe fn alloc(
        &self,
        size: u64,
        alignment: u64,
        memory_type_index: u32,
    ) -> Option<(Block, *mut u8)> {
        if size > self.size {
            return None;
        }

        let mut block_out_index = None;
        {
            let blocks = self.blocks.read().unwrap();
            for (i, block) in blocks.iter().enumerate() {
                if block.is_free {
                    let mut block_size = block.size;
                    if block.offset % alignment != 0 {
                        block_size -= alignment - block.offset % alignment;
                    }

                    if block_size >= size {
                        block_out_index = Some(i);
                        break;
                    }
                }
            }
        }

        // FIXME this is not thread safe: another thread could acquire the lock beetween the release of the read lock and the acquire of the write lock and use our block
        if let Some(i) = block_out_index {
            let mut blocks = self.blocks.write().unwrap();
            trace!("Alloc {} bytes from chunk in block {:?}", size, blocks[i]);

            let before_size = if blocks[i].offset % alignment != 0 {
                alignment - blocks[i].offset % alignment
            } else {
                0
            };

            let after_size = blocks[i].size - (size + before_size);

            if after_size > 0 {
                let new_block = Block::new(
                    self.memory,
                    memory_type_index,
                    blocks[i].offset + size + before_size,
                    after_size,
                );
                blocks.insert(i + 1, new_block);
            }

            let before_block_offset = blocks[i].offset;
            blocks[i].is_free = false;
            blocks[i].size = size;
            blocks[i].offset += before_size;
            let return_block = blocks[i]; // copy here because if we insert a new block before, we should return blocks[i + 1] instead of blocks[i]

            if before_size > 0 {
                let new_block = Block::new(
                    self.memory,
                    memory_type_index,
                    before_block_offset,
                    before_size,
                );
                blocks.insert(i, new_block);
            }

            Some((return_block, self.ptr.add(before_block_offset as usize)))
        } else {
            None
        }
    }

    unsafe fn free(&self, block: Block) {
        trace!("Freeing block {:?}", block);
        let mut blocks = self.blocks.write().unwrap();
        // FIXME binary search
        blocks
            .iter_mut()
            .find(|b| b.offset == block.offset)
            .unwrap()
            .is_free = true;
        // TODO merge adjacent free blocks
    }
}

#[derive(Copy, Clone, Debug)]
pub struct Block {
    pub memory: vk::DeviceMemory,
    memory_type_index: u32,
    pub offset: u64,
    pub size: u64,
    is_free: bool,
}

impl Block {
    fn new(memory: vk::DeviceMemory, memory_type_index: u32, offset: u64, size: u64) -> Self {
        trace!("Creating block at offset {} of {} bytes", offset, size);
        Self {
            memory,
            memory_type_index,
            offset,
            size,
            is_free: true,
        }
    }
}
