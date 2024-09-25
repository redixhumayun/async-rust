#![allow(dead_code)]
use std::{
    cell::RefCell,
    future::Future,
    io::{BufRead, BufReader, Write},
    net::{SocketAddr, TcpStream},
    os::fd::AsRawFd,
    rc::Rc,
};

use log::debug;
use log::error;

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
                cx.waker().wake_by_ref();
                return std::task::Poll::Pending;
            }
            ClientState::Reading => {
                self.state = ClientState::Writing;
                debug!("Reading data from the socket");
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
                    .contains("GET / HTTP 1.1")
                {
                    self.state = ClientState::Writing;
                } else {
                    self.state = ClientState::Close;
                }
                cx.waker().wake_by_ref();
                return std::task::Poll::Pending;
            }
            ClientState::Writing => {
                self.state = ClientState::Close;
                debug!("Writing data back to the socket");
                self.client.write_all(b"Hello from the client").unwrap();
                cx.waker().wake_by_ref();
                return std::task::Poll::Pending;
            }
            ClientState::Close => {
                self.state = ClientState::Closed;
                debug!("Client is now closing, must de-register from reactor etc.");
                // self.reactor
                //     .borrow_mut()
                //     .remove(
                //         self.client.as_raw_fd(),
                //         Event::all(self.client.as_raw_fd().try_into().unwrap()),
                //     )
                //     .expect("Something went wrong while attempting to de-register the client from the reactor");
                cx.waker().wake_by_ref();
                return std::task::Poll::Pending;
            }
            ClientState::Closed => {
                debug!("Client has been closed, returning ready to the executor");
                return std::task::Poll::Ready(Ok(()));
            }
        };
    }
}
