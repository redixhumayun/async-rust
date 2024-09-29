use std::{
    cell::RefCell,
    future::Future,
    io::{Read, Write},
    net::{SocketAddr, TcpStream},
    os::fd::AsRawFd,
    path::PathBuf,
    rc::Rc,
};

use log::debug;

use crate::async_runtime::reactor::{Event, InterestType};

use super::{file_io_pool::FileIOPool, reactor::Reactor};

pub struct TcpClient {
    client: TcpStream,
    _addr: SocketAddr,
    reactor: Rc<RefCell<Reactor>>,
    file_io_pool: Rc<RefCell<FileIOPool>>,
}

impl TcpClient {
    pub fn new(
        client: TcpStream,
        addr: SocketAddr,
        reactor: Rc<RefCell<Reactor>>,
        file_io_pool: Rc<RefCell<FileIOPool>>,
    ) -> Self {
        client.set_nonblocking(true).unwrap();
        Self {
            client,
            _addr: addr,
            reactor,
            file_io_pool,
        }
    }

    pub async fn handle_request(&mut self) -> std::io::Result<()> {
        debug!("handling client request");
        self.read().await?;
        let file_path = PathBuf::from("hello.html");
        let bytes = self
            .file_io_pool
            .borrow()
            .read_file(file_path, Rc::clone(&self.reactor))
            .await
            .expect("Error while reading the response file");
        let mut response = Vec::new();
        let headers = format!(
            "HTTP/1.1 200 OK\r\nContent-Length: {}\r\nConnection: close\r\n\r\n",
            bytes.len()
        );
        response.extend_from_slice(headers.as_bytes());
        response.extend_from_slice(&bytes);
        self.write(response).await?;
        self.client.shutdown(std::net::Shutdown::Write)?;
        Ok(())
    }

    fn read(&self) -> AsyncTcpReader {
        AsyncTcpReader {
            client: &self.client,
            reactor: Rc::clone(&self.reactor),
            buffer: Vec::with_capacity(1024),
            total_read: 0,
        }
    }

    fn write(&self, buffer: Vec<u8>) -> AsyncTcpWriter {
        AsyncTcpWriter {
            client: &self.client,
            reactor: Rc::clone(&self.reactor),
            buffer,
            bytes_written: 0,
        }
    }
}

impl Drop for TcpClient {
    fn drop(&mut self) {
        self.reactor
            .borrow_mut()
            .remove(
                self.client.as_raw_fd(),
                Event::all(self.client.as_raw_fd().try_into().unwrap()),
            )
            .unwrap();
    }
}

pub struct AsyncTcpReader<'read_stream> {
    client: &'read_stream TcpStream,
    reactor: Rc<RefCell<Reactor>>,
    buffer: Vec<u8>,
    total_read: usize,
}

impl<'read_stream> Future for AsyncTcpReader<'read_stream> {
    type Output = std::io::Result<usize>;
    fn poll(
        mut self: std::pin::Pin<&mut Self>,
        cx: &mut std::task::Context<'_>,
    ) -> std::task::Poll<Self::Output> {
        loop {
            let mut chunk = [0u8; 1024];
            match self.client.read(&mut chunk) {
                Ok(0) => {
                    // received EOF
                    self.buffer.clear();
                    return std::task::Poll::Ready(Ok(self.total_read));
                }
                Ok(n) => {
                    self.buffer.extend_from_slice(&chunk);
                    self.total_read += n;
                    let headers = std::str::from_utf8(&chunk[..n]).unwrap();
                    if headers.ends_with("\r\n\r\n") {
                        self.buffer.clear();
                        return std::task::Poll::Ready(Ok(self.total_read));
                    }
                    return std::task::Poll::Pending;
                }
                Err(e) if e.kind() == std::io::ErrorKind::WouldBlock => {
                    self.reactor.borrow_mut().register_interest(
                        self.client.as_raw_fd(),
                        InterestType::Read,
                        cx,
                    );
                    return std::task::Poll::Pending;
                }
                Err(e) => return std::task::Poll::Ready(Err(e)),
            }
        }
    }
}

pub struct AsyncTcpWriter<'write_stream> {
    client: &'write_stream TcpStream,
    reactor: Rc<RefCell<Reactor>>,
    buffer: Vec<u8>,
    bytes_written: usize,
}

impl<'write_stream> Future for AsyncTcpWriter<'write_stream> {
    type Output = std::io::Result<usize>;
    fn poll(
        mut self: std::pin::Pin<&mut Self>,
        cx: &mut std::task::Context<'_>,
    ) -> std::task::Poll<Self::Output> {
        let this = self.as_mut().get_mut();
        loop {
            match this.client.write(&this.buffer[this.bytes_written..]) {
                Ok(n) => {
                    this.bytes_written += n;
                    if this.bytes_written >= this.buffer.len() {
                        return std::task::Poll::Ready(Ok(this.bytes_written));
                    }
                }
                Err(e) if e.kind() == std::io::ErrorKind::WouldBlock => {
                    this.reactor.borrow_mut().register_interest(
                        this.client.as_raw_fd(),
                        InterestType::Write,
                        cx,
                    );
                    return std::task::Poll::Pending;
                }
                Err(e) => {
                    return std::task::Poll::Ready(Err(e));
                }
            }
        }
    }
}
