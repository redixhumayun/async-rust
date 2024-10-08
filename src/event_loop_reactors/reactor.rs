use std::{
    io::{Read, Write},
    os::{
        fd::{AsRawFd, RawFd},
        unix::net::UnixStream,
    },
};

use libc::{c_int, kevent, EVFILT_READ, EVFILT_WRITE, EV_ADD, EV_DELETE, EV_ONESHOT};

#[derive(Debug)]
pub struct Event {
    pub fd: usize,
    pub readable: bool,
    pub writable: bool,
}

impl Event {
    pub fn readable(fd: i32) -> Self {
        Event {
            fd: fd as usize,
            readable: true,
            writable: false,
        }
    }

    pub fn none(fd: i32) -> Self {
        Event {
            fd: fd as usize,
            readable: false,
            writable: false,
        }
    }
}

pub struct Reactor {
    kq: RawFd, //  the kqueue fd
    events: Vec<libc::kevent>,
    capacity: usize,
    notifier: (UnixStream, UnixStream),
}

impl Reactor {
    pub fn new() -> std::io::Result<Self> {
        let kq = unsafe { libc::kqueue() };
        if kq < 0 {
            return Err(std::io::Error::last_os_error());
        }
        let (read_stream, write_stream) = UnixStream::pair()?;
        read_stream.set_nonblocking(true)?;
        write_stream.set_nonblocking(true)?;
        let reactor = Self {
            kq,
            events: Vec::new(),
            capacity: 1,
            notifier: (read_stream, write_stream),
        };
        reactor.modify(
            reactor.notifier.0.as_raw_fd(),
            Event::readable(reactor.notifier.0.as_raw_fd()),
        )?;
        Ok(reactor)
    }

    pub fn add(&mut self, fd: RawFd, ev: Event) -> std::io::Result<()> {
        self.modify(fd, ev)
    }

    pub fn delete(&mut self, fd: RawFd) -> std::io::Result<()> {
        self.modify(fd, Event::none(fd))
    }

    pub fn notify(&mut self) -> std::io::Result<()> {
        self.notifier.1.write(&[1])?;
        Ok(())
    }

    fn modify(&self, fd: RawFd, ev: Event) -> std::io::Result<()> {
        let read_flags = if ev.readable {
            EV_ADD | EV_ONESHOT
        } else {
            EV_DELETE
        };
        let changes = [kevent {
            ident: fd as usize,
            filter: EVFILT_READ,
            flags: read_flags,
            fflags: 0,
            data: 0,
            udata: std::ptr::null_mut(),
        }];
        let result = unsafe {
            kevent(
                self.kq,
                changes.as_ptr(),
                1,
                std::ptr::null_mut(),
                0,
                std::ptr::null_mut(),
            )
        };
        if result < 0 {
            return Err(std::io::Error::last_os_error());
        }
        Ok(())
    }

    pub fn poll(&mut self) -> std::io::Result<Vec<Event>> {
        let max_capacity = self.capacity as c_int;
        self.events.clear();
        let result = unsafe {
            self.events
                .resize(max_capacity as usize, std::mem::zeroed());
            let result = kevent(
                self.kq,
                std::ptr::null(),
                0,
                self.events.as_mut_ptr(),
                max_capacity,
                std::ptr::null(),
            );
            result
        };
        if result < 0 {
            //  return the last OS error
            return Err(std::io::Error::last_os_error());
        }

        let mut mapped_events = Vec::new();
        for i in 0..result as usize {
            let kevent = &self.events[i];
            let ident = kevent.ident;
            let filter = kevent.filter;

            let mut buf = [0; 8];
            if ident == self.notifier.0.as_raw_fd() as usize {
                self.notifier.0.read(&mut buf)?;
                self.modify(
                    self.notifier.0.as_raw_fd(),
                    Event::readable(self.notifier.0.as_raw_fd()),
                )?;
            }

            let mut event = Event {
                fd: ident,
                readable: false,
                writable: false,
            };

            match filter {
                EVFILT_READ => event.readable = true,
                EVFILT_WRITE => event.writable = true,
                _ => {}
            };
            self.modify(event.fd as i32, Event::readable(event.fd as i32))?;
            mapped_events.push(event);
        }
        Ok(mapped_events)
    }
}
