use std::{
    alloc::{dealloc, Layout},
    sync::{Arc, Mutex},
};

use criterion::{criterion_group, criterion_main, BenchmarkId, Criterion, Throughput};
use pretty_env_logger::env_logger::Target;
use vulkan_voxels::{
    render::{
        memory::{AllocRequirements, AllocUsage, Allocator, Block},
        vertex::Vertex,
        Renderer,
    },
    world::{Chunk, ChunkPos},
};

extern crate alloc;
use alloc::alloc::alloc;

use lazy_static::lazy_static;
use vulkanalia::{
    loader::{LibloadingLoader, LIBRARY},
    vk::{self, DeviceV1_0, HasBuilder},
    Entry,
};
use winit::{event_loop::EventLoop, window::WindowBuilder};

unsafe fn create_renderer() -> Arc<Mutex<Renderer>> {
    let event_loop = EventLoop::new();
    let window = WindowBuilder::new()
        .with_visible(false)
        .build(&event_loop)
        .unwrap();

    Arc::new(Mutex::new(Renderer::new(&window, &ENTRY)))
}

lazy_static! {
    static ref ENTRY: Entry = unsafe {
        let loader = LibloadingLoader::new(LIBRARY).unwrap();
        Entry::new(loader).unwrap()
    };
    static ref RENDERER: Arc<Mutex<Renderer>> = {
        pretty_env_logger::formatted_builder()
            .target(Target::Stdout)
            .init();

        unsafe { create_renderer() }
    };
}

fn chunk_bench(c: &mut Criterion) {
    c.bench_function("Mesh chunk", |b| unsafe {
        let mut chunk = Chunk::new(ChunkPos { x: 0, y: 0, z: 0 }).unwrap();
        let layout = Layout::new::<[Vertex; 22000]>();
        let buff1 = alloc(layout);
        let buff2 = alloc(layout);
        b.iter(|| {
            chunk
                .mesh(
                    std::slice::from_raw_parts_mut(buff1.cast(), 22000),
                    std::slice::from_raw_parts_mut(buff2.cast(), 22000),
                )
                .unwrap();
        });
        dealloc(buff1, layout);
        dealloc(buff2, layout);
    });
}

fn alloc_bench(c: &mut Criterion) {
    RENDERER.as_ref();
    let renderer = RENDERER.lock().unwrap();
    let data = renderer.data.read().unwrap();

    static KB: usize = 1024;
    static MAX_ALLOC_SIZE: usize = 2 * KB * KB * KB;

    let mut group = c.benchmark_group("Alloc");
    for size in [16 * KB, 128 * KB, KB * KB, 16 * KB * KB].iter() {
        group.throughput(Throughput::Bytes(*size as u64));
        group.bench_with_input(BenchmarkId::from_parameter(size), size, |b, &size| unsafe {
            let mut allocator =
                Allocator::new(&data.device, &data.instance, data.physical_device.device);
            let info = vk::BufferCreateInfo::builder()
                .size(size as u64)
                .usage(vk::BufferUsageFlags::VERTEX_BUFFER)
                .sharing_mode(vk::SharingMode::EXCLUSIVE);
            let buffer = data.device.create_buffer(&info, None).unwrap();

            struct X<'a>(&'a Allocator, Block);

            impl Drop for X<'_> {
                fn drop(&mut self) {
                    unsafe { self.0.free(self.1) };
                }
            }

            let memory_requirements = data.device.get_buffer_memory_requirements(buffer);
            let requirements = AllocRequirements::new(memory_requirements, AllocUsage::DeviceLocal);
            b.iter_batched(
                || (),
                |_| X(&allocator, allocator.alloc(requirements).unwrap().0),
                criterion::BatchSize::NumIterations((MAX_ALLOC_SIZE / size) as u64),
            );
            allocator.free_all();
        });
    }
    group.finish();
}

criterion_group!(benches, chunk_bench, alloc_bench);
criterion_main!(benches);
