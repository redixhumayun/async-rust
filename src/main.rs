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
    let mut runtime = Executor::new(task_queue, Rc::clone(&reactor)).unwrap();
    runtime.block_on(async move {
        let listener = TcpListener::bind("localhost:8000", Rc::clone(&reactor)).unwrap();
        while let Ok((client, addr)) = listener.accept().unwrap().await {
            let _future_client = TcpClient::new(client, addr, Rc::clone(&reactor)).await;
            debug!("Finished servicing the client, can now resume the loop");
        }
        println!("Hello World");
    });
}
