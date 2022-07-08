use std::{
    sync::{
        atomic::{AtomicBool, Ordering},
        Arc, Mutex, Weak,
    },
    thread,
};

use crossbeam_channel::{Receiver, Sender, TryIter};
use log::trace;

use crate::world::Chunk;

pub struct MeshingThreadPool {
    threads: Vec<thread::JoinHandle<()>>,

    // sender to send chunks to be meshed to the threads
    in_sender: Sender<Weak<Mutex<Chunk>>>,
    in_receiver: Receiver<Weak<Mutex<Chunk>>>,

    // sender to return meshed chunks
    out_sender: Sender<Weak<Mutex<Chunk>>>,
    out_receiver: Receiver<Weak<Mutex<Chunk>>>,

    thread_index_counter: u32,

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
            thread_index_counter: 0,
            exit: Arc::new(AtomicBool::new(false)),
        }
    }

    pub fn start_threads(&mut self, num_threads: usize) {
        trace!("Starting {} meshing threads", num_threads);

        for _ in 0..num_threads {
            let mut name = "Meshing Thread ".to_string();
            name.push_str((self.thread_index_counter).to_string().as_str());

            let sender = self.out_sender.clone();
            let receiver = self.in_receiver.clone();

            let exit = self.exit.clone();

            let thread = thread::Builder::new().name(name).spawn(|| {
                MeshingThreadPool::thread_main(sender, receiver, exit);
            });
            self.threads.push(thread.unwrap());
            self.thread_index_counter += 1;
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

    fn thread_main(
        sender: Sender<Weak<Mutex<Chunk>>>,
        receiver: Receiver<Weak<Mutex<Chunk>>>,
        exit: Arc<AtomicBool>,
    ) {
        trace!("{} started", thread::current().name().unwrap());
        loop {
            if exit.load(Ordering::Relaxed) {
                break;
            }
            let recv_chunk = receiver.recv().unwrap();
            if let Some(chunk) = recv_chunk.upgrade() {
                {
                    let mut chunk = chunk.lock().unwrap();
                    chunk.mesh().unwrap();
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
