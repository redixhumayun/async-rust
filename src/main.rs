use std::{cell::RefCell, rc::Rc};

use log::debug;
use timer_future::async_runtime::{
    client::TcpClient, executor::Executor, file_io_pool::FileIOPool, listener::TcpListener,
    reactor::Reactor, task_queue::TaskQueue,
};

fn main() {
    env_logger::init();
    let task_queue = Rc::new(RefCell::new(TaskQueue::new()));
    let reactor = Rc::new(RefCell::new(Reactor::new().unwrap()));
    let (shutdown_tx, shutdown_rx) = std::sync::mpsc::channel();
    setup_ctrlc_handler(shutdown_tx);
    let file_io_pool = Rc::new(RefCell::new(FileIOPool::new(5, shutdown_rx)));
    let runtime = Rc::new(Executor::new(task_queue, Rc::clone(&reactor)).unwrap());
    let runtime_clone = Rc::clone(&runtime);
    runtime.block_on(async move {
        let listener = TcpListener::bind("localhost:8000", Rc::clone(&reactor)).unwrap();
        while let Ok((client, addr)) = listener.accept().unwrap().await {
            debug!("Received a client connection from client {:?}", client);
            let reactor_clone = Rc::clone(&reactor);
            let file_io_pool_clone = Rc::clone(&file_io_pool);
            runtime_clone.spawn(async move {
                let mut tcp_client =
                    TcpClient::new(client, addr, reactor_clone, Rc::clone(&file_io_pool_clone));
                tcp_client
                    .handle_request()
                    .await
                    .expect("Error occurred while handling the tcp request");
            });
            debug!("Handed off client connection to executor");
        }
    });
    println!("Done executing the top level future");
}

fn setup_ctrlc_handler(shutdown_tx: std::sync::mpsc::Sender<()>) {
    ctrlc::set_handler(move || {
        shutdown_tx
            .send(())
            .expect("failed to send shutdown signal");
    })
    .unwrap();
}
