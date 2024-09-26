#![allow(dead_code)]
use core::future::Future;
use std::{cell::RefCell, rc::Rc, task::Context};

use log::debug;

use super::{
    reactor::{Event, Reactor},
    task_queue::{Task, TaskQueue},
    waker_util::MyWaker,
};

pub struct Executor {
    task_queue: Rc<RefCell<TaskQueue>>,
    reactor: Rc<RefCell<Reactor>>,
    monotonic_clock: usize,
}

impl Executor {
    pub fn new(
        task_queue: Rc<RefCell<TaskQueue>>,
        reactor: Rc<RefCell<Reactor>>,
    ) -> std::io::Result<Self> {
        Ok(Self {
            task_queue,
            reactor,
            monotonic_clock: 0,
        })
    }

    pub fn block_on<F>(&mut self, future: F) -> F::Output
    where
        F: Future<Output = ()> + 'static,
    {
        let task = Task {
            id: self.monotonic_clock,
            future: RefCell::new(Box::pin(future)),
        };
        self.task_queue
            .borrow()
            .sender()
            .send(Rc::new(task))
            .unwrap();
        self.monotonic_clock += 1;
        self.run();
    }

    fn run(&self) {
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

                let waker = MyWaker::new(Rc::clone(&task), self.task_queue.borrow().sender());
                let mut context = Context::from_waker(&waker);
                match task.future.borrow_mut().as_mut().poll(&mut context) {
                    std::task::Poll::Ready(_output) => {
                        debug!(
                            "The future for task {} has completed and returned on thread {:?}",
                            task.id,
                            std::thread::current().id()
                        );
                    }
                    std::task::Poll::Pending => {
                        debug!(
                            "The future for task {} is pending on thread {:?}",
                            task.id,
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
                debug!("waiting on events from the reactor");
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
