use std::{
    cell::RefCell,
    future::Future,
    path::PathBuf,
    rc::Rc,
    sync::{
        mpsc::{channel, Receiver, Sender, TryRecvError},
        Arc, Mutex,
    },
};

use log::debug;

use super::reactor::Reactor;

pub struct FileIOPool {
    sender: Sender<FileReaderTask>,
}

pub struct FileReaderTask {
    pub path: PathBuf,
    pub responder: Sender<std::io::Result<Vec<u8>>>,
}

impl FileIOPool {
    pub fn new(num_threads: usize, shutdown_rx: Receiver<()>) -> Self {
        let (sender, receiver) = channel::<FileReaderTask>();
        let recv = Arc::new(Mutex::new(receiver));
        let shutdown_rx = Arc::new(Mutex::new(shutdown_rx));
        for _ in 0..num_threads {
            let recv_clone = Arc::clone(&recv);
            let shutdown_rx_clone = Arc::clone(&shutdown_rx);
            std::thread::spawn(move || loop {
                let task = recv_clone.lock().unwrap().try_recv();
                let shutdown_signal = shutdown_rx_clone.lock().unwrap().try_recv();
                match (task, shutdown_signal) {
                    (Ok(task), _) => {
                        let result = std::fs::read(task.path);
                        let _ = task.responder.send(result);
                    }
                    (_, Ok(())) => {
                        debug!("File io pool received shutdown signal, shutting down");
                        break;
                    }
                    (Err(_), Err(_)) => {}
                }
            });
        }
        Self { sender }
    }

    pub fn read_file(&self, path: PathBuf, reactor: Rc<RefCell<Reactor>>) -> ReadFileFuture {
        let (file_completion_sender, file_completion_recv) =
            std::sync::mpsc::channel::<std::io::Result<Vec<u8>>>();
        let file_reader_task = FileReaderTask {
            path,
            responder: file_completion_sender,
        };
        self.sender
            .send(file_reader_task)
            .expect("Error while sending the file reader task to io pool");
        ReadFileFuture {
            reactor,
            receiver: file_completion_recv,
        }
    }
}

pub struct ReadFileFuture {
    reactor: Rc<RefCell<Reactor>>,
    receiver: Receiver<std::io::Result<Vec<u8>>>,
}

impl Future for ReadFileFuture {
    type Output = std::io::Result<Vec<u8>>;
    fn poll(
        self: std::pin::Pin<&mut Self>,
        cx: &mut std::task::Context<'_>,
    ) -> std::task::Poll<Self::Output> {
        match self.receiver.try_recv() {
            Ok(data) => return std::task::Poll::Ready(data),
            Err(e) => match e {
                TryRecvError::Empty => {
                    debug!("received empty from the file reader task receiver");
                    self.reactor.borrow_mut().notify()?; //  force the reactor to wake up so scheduler can continue
                    cx.waker().wake_by_ref();
                    return std::task::Poll::Pending;
                }
                TryRecvError::Disconnected => {
                    return std::task::Poll::Ready(Err(std::io::Error::new(
                        std::io::ErrorKind::Other,
                        "Received Disconnected while waiting for file read to complete",
                    )))
                }
            },
        }
    }
}
