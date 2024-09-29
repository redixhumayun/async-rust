use std::{cell::RefCell, future::Future, os::fd::AsRawFd, rc::Rc};

use log::debug;

use super::reactor::{Event, Reactor};

pub struct TcpListener {
    listener: std::net::TcpListener,
    reactor: Rc<RefCell<Reactor>>,
}

impl TcpListener {
    pub fn bind(addr: &str, reactor: Rc<RefCell<Reactor>>) -> std::io::Result<TcpListener> {
        let listener = std::net::TcpListener::bind(addr)?;
        listener.set_nonblocking(true)?;
        Ok(Self { listener, reactor })
    }

    pub fn accept(&self) -> std::io::Result<ListenerFuture> {
        Ok(ListenerFuture {
            listener: &self.listener,
            reactor: Rc::clone(&self.reactor),
        })
    }
}

impl Drop for TcpListener {
    fn drop(&mut self) {
        self.reactor
            .borrow_mut()
            .remove(
                self.listener.as_raw_fd(),
                Event::all(self.listener.as_raw_fd() as _),
            )
            .unwrap();
    }
}

pub struct ListenerFuture<'listener> {
    listener: &'listener std::net::TcpListener,
    reactor: Rc<RefCell<Reactor>>,
}

impl Future for ListenerFuture<'_> {
    type Output = std::io::Result<(std::net::TcpStream, std::net::SocketAddr)>;
    fn poll(
        mut self: std::pin::Pin<&mut Self>,
        cx: &mut std::task::Context<'_>,
    ) -> std::task::Poll<Self::Output> {
        debug!("Received a poll call on the ListenerFuture");
        match self.listener.accept() {
            Ok((stream, addr)) => std::task::Poll::Ready(Ok((stream, addr))),
            Err(e) if e.kind() == std::io::ErrorKind::WouldBlock => {
                debug!("Attempting to accept a connection on the listener WouldBlock, so registering with reactor and yielding control back to the executor");
                let fd = self.listener.as_raw_fd();
                self.as_mut().reactor.borrow_mut().register_interest(
                    fd,
                    super::reactor::InterestType::Read,
                    cx,
                );
                std::task::Poll::Pending
            }
            Err(e) => {
                eprintln!("received an error in the ListenerFuture {}", e);
                std::task::Poll::Ready(Err(e))
            }
        }
    }
}
