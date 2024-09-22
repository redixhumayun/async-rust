use std::{
    future::Future,
    pin::Pin,
    sync::mpsc::{self, Receiver, Sender},
};

pub struct Task {
    future: Pin<Box<dyn Future<Output = ()>>>,
}

pub struct TaskQueue {
    tasks: Vec<Task>,
    sender: Sender<Task>,
    receiver: Receiver<Task>,
}

impl TaskQueue {
    fn new() -> Self {
        let (sender, recv) = mpsc::channel();
        Self {
            tasks: Vec::new(),
            sender,
            receiver: recv,
        }
    }

    fn sender(&self) -> Sender<Task> {
        self.sender.clone()
    }

    fn receive(&mut self) {
        while let Ok(task) = self.receiver.try_recv() {
            self.tasks.push(task);
        }
    }

    fn pop(&mut self) -> Option<Task> {
        self.tasks.pop()
    }
}
