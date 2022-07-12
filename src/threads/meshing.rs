use std::{
    mem::size_of,
    sync::{
        atomic::{AtomicBool, Ordering},
        Arc, Mutex, RwLock, Weak,
    },
    thread,
};

use crossbeam_channel::{Receiver, Sender, TryIter};
use log::trace;
use vulkanalia::vk::{self, DeviceV1_0, Handle, HasBuilder};

use crate::{
    config::CHUNK_SIZE,
    render::{
        buffer::Buffer, commands::CommandPool, memory::AllocUsage, queue::QueueFamilyIndices,
        renderer::RendererData, vertex::Vertex,
    },
    world::Chunk,
};

pub const MESHING_THREADS_COUNT: usize = 4;
pub const STAGING_BUFFER_SIZE: usize =
    ((CHUNK_SIZE * CHUNK_SIZE * CHUNK_SIZE) as usize * size_of::<Vertex>() * 36) / 2;

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
        trace!("Starting {} meshing threads", MESHING_THREADS_COUNT);

        for i in 0..MESHING_THREADS_COUNT {
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
        trace!("{} started", thread::current().name().unwrap());
        let (staging_buffer, queue_index, queue) = {
            let data = renderer_data.read().unwrap();
            let staging_buffer = Buffer::create(
                &data,
                STAGING_BUFFER_SIZE,
                vk::BufferUsageFlags::TRANSFER_SRC,
                AllocUsage::Staging,
            )
            .unwrap();

            let indices =
                QueueFamilyIndices::get(&data.instance, data.surface, data.physical_device)
                    .unwrap();
            let queue = data.device.as_ref().get_device_queue(indices.transfer, i);

            (staging_buffer, indices.transfer, queue)
        };

        let command_pool =
            CommandPool::create(&renderer_data.read().unwrap(), queue_index).unwrap();
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
                    chunk
                        .mesh(&renderer_data.read().unwrap(), staging_buffer.ptr.cast())
                        .unwrap();

                    let device = &renderer_data.read().unwrap().device;
                    command_buffer.begin(device).unwrap();
                    let regions = vk::BufferCopy::builder()
                        .size((chunk.vertices_len * std::mem::size_of::<Vertex>()) as u64)
                        .build();
                    device.cmd_copy_buffer(
                        command_buffer.buffer,
                        staging_buffer.buffer,
                        chunk.vertex_buffer.as_ref().unwrap().buffer,
                        &[regions],
                    );
                    command_buffer.end(device).unwrap();

                    let submit_info = vk::SubmitInfo::builder()
                        .command_buffers(&[command_buffer.buffer])
                        .build();
                    device
                        .queue_submit(queue, &[submit_info], vk::Fence::null())
                        .unwrap();
                    device.queue_wait_idle(queue).unwrap();
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
