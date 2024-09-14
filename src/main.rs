use std::os::{
    fd::{AsRawFd, RawFd},
    raw::c_int,
};

use libc::{kevent, EVFILT_READ, EVFILT_WRITE, EV_ADD, EV_DELETE, EV_ENABLE, EV_ONESHOT};

#[derive(Debug)]
struct Event {
    fd: usize,
    readable: bool,
    writable: bool,
}

impl Event {
    fn readable(fd: i32) -> Self {
        Event {
            fd: fd as usize,
            readable: true,
            writable: false,
        }
    }
}

struct Reactor {
    kq: RawFd, //  the kqueue fd
    events: Vec<libc::kevent>,
    capacity: usize,
}

impl Reactor {
    pub fn new() -> std::io::Result<Self> {
        let kq = unsafe { libc::kqueue() };
        if kq < 0 {
            return Err(std::io::Error::last_os_error());
        }
        Ok(Self {
            kq,
            events: Vec::new(),
            capacity: 1,
        })
    }

    pub fn add(&mut self, fd: RawFd, ev: Event) -> std::io::Result<()> {
        self.modify(fd, ev)
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

fn main() {
    let mut reactor = Reactor::new().unwrap();
    let tcp_listener = std::net::TcpListener::bind("127.0.0.1:8000").unwrap();
    let raw_fd = tcp_listener.as_raw_fd();
    reactor.add(raw_fd, Event::readable(raw_fd)).unwrap();
    loop {
        let events = reactor.poll().unwrap();
        println!("the events received {:?}", events);
        for event in events {
            if event.readable {
                match tcp_listener.accept() {
                    Ok((mut socket, addr)) => {
                        println!("received new connection");
                    }
                    Err(e) => eprintln!("Error accepting connection: {}", e),
                }
            }
        }
    }
}
