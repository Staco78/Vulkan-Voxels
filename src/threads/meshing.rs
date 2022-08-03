use std::{
    mem::size_of,
    num::NonZeroUsize,
    sync::{
        atomic::{AtomicBool, Ordering},
        Arc, Mutex, RwLock, Weak,
    },
    thread,
};

use crossbeam_channel::{Receiver, Sender, TryIter};
use log::{info, trace, warn};
use vulkanalia::vk::{self, DeviceV1_0, Handle, HasBuilder};

use crate::{
    config::CHUNK_SIZE,
    render::{
        buffer::Buffer, commands::CommandPool, memory::AllocUsage, physical_device::PhysicalDevice,
        renderer::RendererData, vertex::Vertex,
    },
    world::Chunk,
};

pub const STAGING_BUFFER_SIZE_VERTICES: usize =
    ((CHUNK_SIZE * CHUNK_SIZE * CHUNK_SIZE) as usize * size_of::<Vertex>() * 36) / 5;
pub const STAGING_BUFFER_SIZE_INDICES: usize =
    ((CHUNK_SIZE * CHUNK_SIZE * CHUNK_SIZE) as usize * 36) * 2;

#[inline]
fn get_threads_count(physical_device: &PhysicalDevice) -> usize {
    let parallelism: usize = thread::available_parallelism()
        .unwrap_or_else(|_| {
            warn!("Unable to know the CPU cores count: default to 4");
            unsafe { NonZeroUsize::new_unchecked(4) }
        })
        .into();
    info!("Detected {parallelism} cores");
    let max_meshing_threads = match parallelism {
        1..=4 => parallelism,
        _ => parallelism - 1,
    };
    max_meshing_threads.min(physical_device.transfer_queues.len())
}

pub struct MeshingThreadPool {
    threads: Vec<thread::JoinHandle<()>>,

    // sender to send chunks to be meshed to the threads
    in_sender: Sender<Weak<Mutex<Chunk>>>,
    in_receiver: Receiver<Weak<Mutex<Chunk>>>,

    // sender to return meshed chunks
    out_sender: Sender<Weak<Mutex<Chunk>>>,
    out_receiver: Receiver<Weak<Mutex<Chunk>>>,

    exit: Arc<AtomicBool>,
}

impl MeshingThreadPool {
    pub fn new() -> Self {
        let (in_sender, in_receiver) = crossbeam_channel::unbounded();
        let (out_sender, out_receiver) = crossbeam_channel::unbounded();

        Self {
            threads: Vec::new(),
            in_sender,
            in_receiver,
            out_sender,
            out_receiver,
            exit: Arc::new(AtomicBool::new(false)),
        }
    }

    pub unsafe fn start_threads(&mut self, data: Arc<RwLock<RendererData>>) {
        let threads_count = get_threads_count(&data.read().unwrap().physical_device);
        info!("Starting {} meshing threads", threads_count);

        for i in 0..threads_count {
            let mut name = "Meshing Thread ".to_string();
            name.push_str(i.to_string().as_str());

            let sender = self.out_sender.clone();
            let receiver = self.in_receiver.clone();

            let exit = self.exit.clone();

            let data = data.clone();

            let thread = thread::Builder::new().name(name).spawn(move || {
                MeshingThreadPool::thread_main(i as u32, sender, receiver, exit, data);
            });
            self.threads.push(thread.unwrap());
        }
    }

    pub fn exit_all(&mut self) {
        self.exit.store(true, Ordering::Relaxed);

        // send a empty weak to all threads to prevent them from blocking on the in_receiver
        for _ in 0..self.threads.len() {
            self.in_sender.send(Weak::new()).unwrap();
        }
        for _ in 0..self.threads.len() {
            self.threads.pop().unwrap().join().unwrap();
        }
    }

    pub fn mesh_thread(&self, chunk: Weak<Mutex<Chunk>>) {
        self.in_sender.send(chunk).unwrap();
    }

    unsafe fn thread_main(
        i: u32,
        sender: Sender<Weak<Mutex<Chunk>>>,
        receiver: Receiver<Weak<Mutex<Chunk>>>,
        exit: Arc<AtomicBool>,
        renderer_data: Arc<RwLock<RendererData>>,
    ) {
        profiling::register_thread!();
        trace!("{} started", thread::current().name().unwrap());
        let (staging_buffer, queue_family, queue) = {
            let data = renderer_data.read().unwrap();
            let staging_buffer = Buffer::create(
                &data,
                STAGING_BUFFER_SIZE_VERTICES + STAGING_BUFFER_SIZE_INDICES,
                vk::BufferUsageFlags::TRANSFER_SRC,
                AllocUsage::Staging,
            )
            .unwrap();

            let queue_def = renderer_data
                .read()
                .unwrap()
                .physical_device
                .transfer_queues[i as usize];
            let queue = data
                .device
                .as_ref()
                .get_device_queue(queue_def.family, queue_def.index);

            (staging_buffer, queue_def.family, queue)
        };

        let command_pool =
            CommandPool::create(&renderer_data.read().unwrap(), queue_family).unwrap();
        let mut command_buffer = command_pool
            .allocate_command_buffers(&renderer_data.read().unwrap().device, 1)
            .unwrap()[0];

        loop {
            if exit.load(Ordering::Relaxed) {
                break;
            }
            let recv_chunk = receiver.recv().unwrap();
            if let Some(chunk) = recv_chunk.upgrade() {
                {
                    let mut chunk = chunk.lock().unwrap();
                    {
                        profiling::scope!("meshing");
                        chunk
                            .mesh(
                                std::slice::from_raw_parts_mut(
                                    staging_buffer.ptr.cast(),
                                    STAGING_BUFFER_SIZE_VERTICES,
                                ),
                                std::slice::from_raw_parts_mut(
                                    staging_buffer.ptr.add(STAGING_BUFFER_SIZE_VERTICES).cast(),
                                    STAGING_BUFFER_SIZE_INDICES,
                                ),
                            )
                            .unwrap();
                        chunk.buffer = Some(Buffer::create(
                            &renderer_data.read().unwrap(),
                            chunk.vertices_count * size_of::<Vertex>()
                                + chunk.indices_count * size_of::<u32>(),
                            vk::BufferUsageFlags::VERTEX_BUFFER
                                | vk::BufferUsageFlags::INDEX_BUFFER
                                | vk::BufferUsageFlags::TRANSFER_DST,
                            AllocUsage::DeviceLocal,
                        ).unwrap());
                    }

                    {
                        profiling::scope!("uploading");
                        let device = &renderer_data.read().unwrap().device;
                        {
                            profiling::scope!("recording");
                            command_buffer.begin(device).unwrap();
                            let regions = [
                                vk::BufferCopy::builder().size(
                                    (chunk.vertices_count * std::mem::size_of::<Vertex>()) as u64,
                                ),
                                vk::BufferCopy::builder()
                                    .src_offset(STAGING_BUFFER_SIZE_VERTICES as u64)
                                    .dst_offset(
                                        (chunk.vertices_count * std::mem::size_of::<Vertex>())
                                            as u64,
                                    )
                                    .size(
                                        (chunk.indices_count * std::mem::size_of::<u32>()) as u64,
                                    ),
                            ];
                            device.cmd_copy_buffer(
                                command_buffer.buffer,
                                staging_buffer.buffer,
                                chunk.buffer.as_ref().unwrap().buffer,
                                &regions,
                            );

                            command_buffer.end(device).unwrap();
                        }

                        {
                            profiling::scope!("submitting");
                            let buffers = &[command_buffer.buffer];
                            let submit_info = vk::SubmitInfo::builder().command_buffers(buffers);
                            device
                                .queue_submit(queue, &[submit_info], vk::Fence::null())
                                .unwrap();
                        }
                        profiling::scope!("waiting");
                        device.queue_wait_idle(queue).unwrap();
                    }
                }
                sender.send(recv_chunk).unwrap();
            }
        }
        trace!("{} exited", thread::current().name().unwrap());
    }

    pub fn try_iter(&self) -> TryIter<'_, Weak<Mutex<Chunk>>> {
        self.out_receiver.try_iter()
    }
}
