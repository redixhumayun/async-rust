#[allow(dead_code)]
use std::{
    collections::HashMap,
    net::{TcpListener, TcpStream},
    os::fd::AsRawFd,
};
use std::{
    io::{BufRead, BufReader, Write},
    path::Path,
    sync::{Arc, Mutex},
};

use reactor::{Event, Reactor};
use task_queue::{RegistrationTask, ScheduledTask, Task, TaskQueue, UnregistrationTask};
mod reactor;
mod task_queue;

trait EventHandler {
    fn event(&mut self, event: Event);
    fn poll(&mut self);
}

#[derive(Debug)]
enum AsyncTcpClientState {
    Waiting,
    Reading,
    Writing,
    Close,
    Closed,
}

struct AsyncTcpClient {
    client: TcpStream,
    fd: usize,
    reactor: Arc<Mutex<Reactor>>,
    task_queue: Arc<Mutex<TaskQueue>>,
    state: Option<AsyncTcpClientState>,
}

impl AsyncTcpClient {
    fn new(
        client: TcpStream,
        reactor: Arc<Mutex<Reactor>>,
        task_queue: Arc<Mutex<TaskQueue>>,
    ) -> std::io::Result<Self> {
        let fd = client.as_raw_fd();
        reactor.lock().unwrap().add(fd, Event::readable(fd))?;
        Ok(Self {
            client,
            fd: fd as usize,
            reactor,
            task_queue,
            state: Some(AsyncTcpClientState::Waiting),
        })
    }
}

impl EventHandler for AsyncTcpClient {
    fn event(&mut self, event: Event) {
        match self.state.take() {
            Some(AsyncTcpClientState::Waiting) => {
                if event.readable {
                    self.state.replace(AsyncTcpClientState::Reading);
                    self.task_queue
                        .lock()
                        .unwrap()
                        .add_task(Task::ScheduledTask(ScheduledTask { fd: self.fd }));
                }
            }
            Some(s) => {
                self.state.replace(s);
            }
            None => {
                panic!("state was none");
            }
        }
    }

    fn poll(&mut self) {
        match self.state.take() {
            None => {}
            Some(AsyncTcpClientState::Waiting) => {
                panic!("The Waiting state should not be reached in the poll fn for client")
            }
            Some(AsyncTcpClientState::Reading) => {
                println!("reading the data from the client into a buffer");
                let reader = BufReader::new(&self.client);
                let http_request: Vec<_> = reader
                    .lines()
                    .map(|line| line.unwrap())
                    .take_while(|line| !line.is_empty())
                    .collect();
                if http_request
                    .iter()
                    .next()
                    .unwrap()
                    .contains("GET / HTTP/1.1")
                {
                    self.state.replace(AsyncTcpClientState::Writing);
                } else {
                    eprintln!("received invalid request, closing the socket connection");
                    self.state.replace(AsyncTcpClientState::Close);
                }
                println!("received request {:?}", http_request);
                self.state.replace(AsyncTcpClientState::Writing);
                self.task_queue
                    .lock()
                    .unwrap()
                    .add_task(Task::ScheduledTask(ScheduledTask { fd: self.fd }));
                self.reactor.lock().unwrap().notify().unwrap();
            }
            Some(AsyncTcpClientState::Writing) => {
                println!("writing the data back into the stream");
                let path = Path::new("hello.html");
                let content = std::fs::read(path).unwrap();
                let response = format!(
                    "HTTP/1.1 200 OK\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{}",
                    content.len(),
                    String::from_utf8_lossy(&content)
                );
                self.client.write_all(response.as_bytes()).unwrap();
                self.state.replace(AsyncTcpClientState::Close);
                self.task_queue
                    .lock()
                    .unwrap()
                    .add_task(Task::ScheduledTask(ScheduledTask { fd: self.fd }));
                self.reactor.lock().unwrap().notify().unwrap();
            }
            Some(AsyncTcpClientState::Close) => {
                println!("closing the client socket connection");
                //  remove the client fd from the reactor and unregister from the event loop
                self.reactor
                    .lock()
                    .unwrap()
                    .delete(self.fd.try_into().unwrap())
                    .unwrap();
                self.task_queue
                    .lock()
                    .unwrap()
                    .add_task(Task::UnregistrationTask(UnregistrationTask { fd: self.fd }));
                self.client.shutdown(std::net::Shutdown::Both).unwrap();
                self.state.replace(AsyncTcpClientState::Closed);
            }
            Some(AsyncTcpClientState::Closed) => {}
        }
    }
}

enum AsyncTcpListenerState {
    WaitingForConnection,
    Accepting(TcpStream),
}

struct AsyncTcpListener {
    listener: TcpListener,
    fd: usize,
    reactor: Arc<Mutex<Reactor>>,
    task_queue: Arc<Mutex<TaskQueue>>,
    state: Option<AsyncTcpListenerState>,
}

impl AsyncTcpListener {
    fn new(
        listener: TcpListener,
        reactor: Arc<Mutex<Reactor>>,
        task_queue: Arc<Mutex<TaskQueue>>,
    ) -> std::io::Result<Self> {
        let fd = listener.as_raw_fd();
        reactor.lock().unwrap().add(fd, Event::readable(fd))?;
        Ok(AsyncTcpListener {
            listener,
            fd: fd as usize,
            reactor,
            task_queue,
            state: Some(AsyncTcpListenerState::WaitingForConnection),
        })
    }
}

impl EventHandler for AsyncTcpListener {
    fn event(&mut self, event: Event) {
        println!("received event on AsyncTcpListener {:?}", event);
        match event.readable {
            true => match self.listener.accept() {
                Ok((client, addr)) => {
                    println!("received client connection from addr {addr}");
                    self.state.replace(AsyncTcpListenerState::Accepting(client));
                    self.task_queue
                        .lock()
                        .unwrap()
                        .add_task(Task::ScheduledTask(ScheduledTask { fd: self.fd }));
                }
                Err(e) => eprintln!("Error accepting connection: {}", e),
            },
            false => {
                panic!("AsyncTcpListener received an event that is not readable")
            }
        }
    }

    fn poll(&mut self) {
        match self.state.take() {
            Some(AsyncTcpListenerState::Accepting(client)) => {
                let client = AsyncTcpClient::new(
                    client,
                    Arc::clone(&self.reactor),
                    Arc::clone(&self.task_queue),
                )
                .unwrap();
                self.task_queue
                    .lock()
                    .unwrap()
                    .add_task(Task::RegistrationTask(RegistrationTask {
                        fd: client.fd,
                        reference: Box::new(client),
                    }));
            }
            Some(AsyncTcpListenerState::WaitingForConnection) => {
                panic!("The WaitingForConnection state should not be reached in the poll fn for listener")
            }
            None => {
                panic!("No state found in the poll fn for listener")
            }
        }
    }
}

struct EventLoop {
    reactor: Arc<Mutex<Reactor>>,
    task_queue: Arc<Mutex<TaskQueue>>,
    references: HashMap<usize, Box<dyn EventHandler>>,
}

impl EventLoop {
    fn new(reactor: Arc<Mutex<Reactor>>, task_queue: Arc<Mutex<TaskQueue>>) -> Self {
        Self {
            reactor,
            task_queue,
            references: HashMap::new(),
        }
    }

    /// Add a reference to the object backing the file descriptor
    fn register(&mut self, fd: usize, reference: Box<dyn EventHandler>) {
        self.references.insert(fd, reference);
    }

    /// Remove the reference backing the file descriptor
    fn unregister(&mut self, fd: usize) {
        self.references.remove(&fd);
    }

    fn process_tasks(&mut self) {
        let mut tasks_to_process = Vec::new();

        {
            // Collect tasks to process
            let mut task_queue = self.task_queue.lock().unwrap();
            while let Some(task) = task_queue.queue.pop() {
                tasks_to_process.push(task);
            }
        }

        // Process collected tasks
        for task in tasks_to_process {
            match task {
                Task::RegistrationTask(registration_task) => {
                    self.register(registration_task.fd, registration_task.reference);
                }
                Task::UnregistrationTask(unregistration_task) => {
                    self.unregister(unregistration_task.fd);
                }
                Task::ScheduledTask(scheduled_task) => {
                    if let Some(reference) = self.references.get_mut(&scheduled_task.fd) {
                        reference.poll();
                    }
                }
            }
        }
    }

    fn handle_events(&mut self, events: Vec<Event>) {
        for event in events {
            if let Some(reference) = self.references.get_mut(&event.fd) {
                reference.event(event);
            }
        }
    }

    fn run(&mut self) {
        loop {
            self.process_tasks();
            let events = self
                .reactor
                .lock()
                .unwrap()
                .poll()
                .expect("Error polling the reactor");

            self.handle_events(events);
        }
    }
}

fn main() {
    let reactor = Arc::new(Mutex::new(Reactor::new().unwrap()));
    let task_queue = Arc::new(Mutex::new(TaskQueue::new()));
    let mut event_loop = EventLoop::new(Arc::clone(&reactor), Arc::clone(&task_queue));

    //  start listener
    let tcp_listener = TcpListener::bind("127.0.0.1:8000").unwrap();
    let listener =
        AsyncTcpListener::new(tcp_listener, Arc::clone(&reactor), Arc::clone(&task_queue)).unwrap();
    task_queue
        .lock()
        .unwrap()
        .add_task(Task::RegistrationTask(RegistrationTask {
            fd: listener.fd,
            reference: Box::new(listener),
        }));

    //  start the event loop
    event_loop.run();
}
