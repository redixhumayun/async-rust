#![allow(dead_code)]
use std::{
    cell::RefCell,
    future::Future,
    io::{BufRead, BufReader, Read, Write},
    net::{SocketAddr, TcpStream},
    os::fd::AsRawFd,
    path::Path,
    rc::Rc,
};

use log::debug;

use crate::async_runtime::reactor::InterestType;

use super::reactor::Reactor;

enum ClientState {
    Waiting,
    Reading,
    Writing,
    Close,
    Closed,
}

pub struct TcpClient {
    client: TcpStream,
    addr: SocketAddr,
    reactor: Rc<RefCell<Reactor>>,
    state: ClientState,
}

impl TcpClient {
    pub fn new(client: TcpStream, addr: SocketAddr, reactor: Rc<RefCell<Reactor>>) -> TcpClient {
        TcpClient {
            client,
            addr,
            reactor,
            state: ClientState::Waiting,
        }
    }
}

impl Future for TcpClient {
    type Output = std::io::Result<()>;
    fn poll(
        mut self: std::pin::Pin<&mut Self>,
        cx: &mut std::task::Context<'_>,
    ) -> std::task::Poll<Self::Output> {
        match self.state {
            ClientState::Waiting => {
                debug!("starting the client state machine");
                self.reactor.borrow_mut().register_interest(
                    self.client.as_raw_fd(),
                    InterestType::Read,
                    cx,
                );
                self.state = ClientState::Reading;
                return std::task::Poll::Pending;
            }
            ClientState::Reading => {
                debug!("Reading data from the socket");
                std::thread::sleep(std::time::Duration::from_secs(2));
                self.state = ClientState::Writing;
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
                    self.state = ClientState::Writing;
                } else {
                    self.state = ClientState::Close;
                }
                cx.waker().wake_by_ref();
                self.reactor.borrow_mut().notify().unwrap();
                return std::task::Poll::Pending;
            }
            ClientState::Writing => {
                debug!("Writing from the client");
                let path = Path::new("hello.html");
                let file = std::fs::File::open(path).unwrap();
                let mut reader = BufReader::new(file);
                let mut buf = Vec::new();
                reader.read_to_end(&mut buf).unwrap();
                let response = format!(
                    "HTTP/1.1 200 OK\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{}",
                    buf.len(),
                    String::from_utf8_lossy(&buf),
                );
                self.client.write_all(response.as_bytes()).unwrap();
                self.state = ClientState::Close;
                cx.waker().wake_by_ref();
                self.reactor.borrow_mut().notify().unwrap();
                return std::task::Poll::Pending;
            }
            ClientState::Close => {
                debug!("Client is now closing, must de-register from reactor etc.");
                self.state = ClientState::Closed;
                cx.waker().wake_by_ref();
                self.reactor.borrow_mut().notify().unwrap();
                return std::task::Poll::Pending;
            }
            ClientState::Closed => {
                debug!("Client has been closed, returning ready to the executor");
                return std::task::Poll::Ready(Ok(()));
            }
        };
    }
}
