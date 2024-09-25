#![allow(dead_code)]
use core::future::Future;
use std::{cell::RefCell, rc::Rc, task::Context};

use log::{debug, info};

use super::{
    reactor::{Event, Reactor},
    task_queue::{Task, TaskQueue},
    waker_util::MyWaker,
};

pub struct Executor {
    task_queue: Rc<RefCell<TaskQueue>>,
    reactor: Rc<RefCell<Reactor>>,
}

impl Executor {
    pub fn new(
        task_queue: Rc<RefCell<TaskQueue>>,
        reactor: Rc<RefCell<Reactor>>,
    ) -> std::io::Result<Self> {
        Ok(Self {
            task_queue,
            reactor,
        })
    }

    pub fn block_on<F>(&self, future: F) -> F::Output
    where
        F: Future<Output = ()> + 'static,
    {
        debug!("Received a future to block_on in the executor");
        let task = Task {
            future: RefCell::new(Box::pin(future)),
        };
        self.task_queue
            .borrow()
            .sender()
            .send(Rc::new(task))
            .unwrap();
        debug!("Pushed the future on the task queue");
        self.run();
    }

    fn run(&self) {
        debug!("Running the executor's event loop");
        self.task_queue.borrow_mut().receive(); //  prime the event loop with the first set of tasks
        loop {
            loop {
                let task = {
                    if let Some(task) = self.task_queue.borrow_mut().pop() {
                        task
                    } else {
                        break;
                    }
                };
                debug!("Received a task in the event loop");

                let waker = MyWaker::new(Rc::clone(&task), self.task_queue.borrow().sender());
                let mut context = Context::from_waker(&waker);
                debug!("Calling poll on the future in the task");
                match task.future.borrow_mut().as_mut().poll(&mut context) {
                    std::task::Poll::Ready(_output) => {
                        debug!(
                            "The future has completed and returned on thread {:?}",
                            std::thread::current().id()
                        );
                    }
                    std::task::Poll::Pending => {
                        debug!(
                            "The future is pending on thread {:?}",
                            std::thread::current().id()
                        );
                    }
                };
            }

            self.task_queue.borrow_mut().receive();
            if !self.reactor.borrow().waiting_on_events() && self.task_queue.borrow().is_empty() {
                debug!("no events to wait on and no events in the queue, so breaking out");
                break;
            }

            if self.reactor.borrow().waiting_on_events() {
                self.wait_for_io()
                    .map(|events| self.wake_futures_on_io(events))
                    .expect("Error while waiting for io");
            }
        }
    }

    fn wait_for_io(&self) -> std::io::Result<Vec<Event>> {
        self.reactor.borrow_mut().poll()
    }

    fn wake_futures_on_io(&self, events: Vec<Event>) {
        let wakers = self.reactor.borrow_mut().get_wakers(events);
        let _ = wakers
            .into_iter()
            .map(|waker| waker.wake())
            .collect::<Vec<_>>();
    }
}
