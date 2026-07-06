use std::{
    sync::{Arc, Mutex, mpsc},
    thread,
};

type Job = Box<dyn FnOnce() + Send + 'static>;
type Receiver = Arc<Mutex<mpsc::Receiver<Job>>>;

pub struct ThreadPool {
    workers: Vec<Worker>,
    sender: Option<mpsc::SyncSender<Job>>,
}

impl ThreadPool {
    pub fn new(num_thread: usize) -> Self {
        assert!(num_thread > 0);
        let (sender, receiver) = mpsc::sync_channel::<Job>(num_thread * 2);
        let receiver = Arc::new(Mutex::new(receiver));
        let workers = (0..num_thread)
            .map(|i| Worker::new(i, receiver.clone()))
            .collect();

        Self {
            workers,
            sender: Some(sender),
        }
    }

    pub fn execute<F>(&self, job: F)
    where
        F: FnOnce() + Send + 'static,
    {
        self.sender.as_ref().unwrap().send(Box::new(job)).unwrap();
    }
}

impl Drop for ThreadPool {
    fn drop(&mut self) {
        self.sender.take();
        for worker in self.workers.drain(..) {
            worker.handle.join().unwrap();
        }
    }
}

struct Worker {
    handle: thread::JoinHandle<()>,
}

impl Worker {
    fn new(id: usize, receiver: Receiver) -> Self {
        let handle = thread::Builder::new()
            .name(format!("dupe-worker-{id}"))
            .spawn(move || {
                loop {
                    let job = receiver.lock().unwrap().recv();
                    match job {
                        Ok(job) => job(),
                        Err(_) => break,
                    }
                }
            })
            .expect("failed to spawn worker thread");
        Self { handle }
    }
}
