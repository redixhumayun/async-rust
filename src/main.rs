use std::{cell::RefCell, rc::Rc};

use log::debug;
use timer_future::async_runtime::{
    client::TcpClient, executor::Executor, listener::TcpListener, reactor::Reactor,
    task_queue::TaskQueue,
};

fn main() {
    env_logger::init();
    let task_queue = Rc::new(RefCell::new(TaskQueue::new()));
    let reactor = Rc::new(RefCell::new(Reactor::new().unwrap()));
    let runtime = Executor::new(task_queue, Rc::clone(&reactor)).unwrap();
    runtime.block_on(async move {
        let listener = TcpListener::bind("localhost:8000", reactor).unwrap();
        debug!("Listening on port 8000");
        while let Ok((client, addr)) = listener.accept().unwrap().await {
            println!("received connection from {:#?}", client);
            let future_client = TcpClient::new(client, addr).await;
        }
        println!("Hello World");
    });
}
