#![allow(dead_code)]
use core::future::Future;
use std::{cell::RefCell, rc::Rc, sync::Mutex, task::Context};

use log::debug;

use super::{
    reactor::{Event, Reactor},
    task_queue::{Task, TaskQueue},
    waker_util::MyWaker,
};

pub struct Executor {
    task_queue: Rc<RefCell<TaskQueue>>,
    reactor: Rc<RefCell<Reactor>>,
    monotonic_clock: Mutex<usize>,
}

impl Executor {
    pub fn new(
        task_queue: Rc<RefCell<TaskQueue>>,
        reactor: Rc<RefCell<Reactor>>,
    ) -> std::io::Result<Self> {
        Ok(Self {
            task_queue,
            reactor,
            monotonic_clock: Mutex::new(0),
        })
    }

    pub fn block_on<F>(&self, future: F) -> F::Output
    where
        F: Future<Output = ()> + 'static,
    {
        let task = Task {
            id: *self.monotonic_clock.lock().unwrap(),
            future: RefCell::new(Box::pin(future)),
        };
        *self.monotonic_clock.lock().unwrap() += 1;
        self.task_queue
            .borrow()
            .sender()
            .send(Rc::new(task))
            .unwrap();
        self.run();
    }

    pub fn spawn<F>(&self, future: F) -> F::Output
    where
        F: Future<Output = ()> + 'static,
    {
        let task = Task {
            id: *self.monotonic_clock.lock().unwrap(),
            future: RefCell::new(Box::pin(future)),
        };
        *self.monotonic_clock.lock().unwrap() += 1;
        self.task_queue
            .borrow()
            .sender()
            .send(Rc::new(task))
            .unwrap();
        self.reactor.borrow_mut().notify().unwrap();
    }

    fn run(&self) {
        loop {
            self.task_queue.borrow_mut().receive();
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
                match self.wait_for_io() {
                    Ok(events) => self.wake_futures_on_io(events),
                    Err(e) => {
                        if e.kind() == std::io::ErrorKind::Interrupted {
                            break;
                        }
                        eprintln!("Error while waiting for IO events :{}", e);
                    }
                }
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
