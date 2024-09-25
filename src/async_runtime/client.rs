#![allow(dead_code)]
use std::{
    future::Future,
    io::Write,
    net::{SocketAddr, TcpStream},
};

use log::debug;

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
    state: ClientState,
}

impl TcpClient {
    pub fn new(client: TcpStream, addr: SocketAddr) -> TcpClient {
        TcpClient {
            client,
            addr,
            state: ClientState::Waiting,
        }
    }
}

impl Future for TcpClient {
    type Output = ();
    fn poll(
        mut self: std::pin::Pin<&mut Self>,
        cx: &mut std::task::Context<'_>,
    ) -> std::task::Poll<Self::Output> {
        match self.state {
            ClientState::Waiting => {
                debug!("starting the client state machine");
                self.state = ClientState::Reading;
                cx.waker().wake_by_ref();
                return std::task::Poll::Pending;
            }
            ClientState::Reading => {
                self.state = ClientState::Writing;
                debug!("Reading data from the socket");
                cx.waker().wake_by_ref();
                return std::task::Poll::Pending;
            }
            ClientState::Writing => {
                self.state = ClientState::Close;
                debug!("Writing data back to the socket");
                self.client.write_all(b"Hello from the client").unwrap();
                return std::task::Poll::Pending;
            }
            ClientState::Close => {
                self.state = ClientState::Closed;
                debug!("Client is now closining, must de-register from reactor etc.");
                return std::task::Poll::Pending;
            }
            ClientState::Closed => {
                return std::task::Poll::Ready(());
            }
        };
    }
}
