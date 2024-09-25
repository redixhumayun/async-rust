#![allow(dead_code)]
use std::{
    collections::HashMap,
    ffi::c_void,
    io::{Read, Write},
    os::{
        fd::{AsRawFd, RawFd},
        unix::net::UnixStream,
    },
    task::{Context, Waker},
};

use libc::{kevent, EVFILT_READ, EVFILT_WRITE, EV_ADD, EV_DELETE, EV_ONESHOT};
use log::debug;

#[derive(Debug)]
pub struct Event {
    pub fd: usize,
    pub readable: bool,
    pub writable: bool,
}

impl Event {
    pub fn none(fd: usize) -> Event {
        Event {
            fd,
            readable: false,
            writable: false,
        }
    }

    pub fn readable(fd: usize) -> Event {
        Event {
            fd,
            readable: true,
            writable: false,
        }
    }

    pub fn writable(fd: usize) -> Event {
        Event {
            fd,
            readable: false,
            writable: true,
        }
    }

    pub fn all(fd: usize) -> Event {
        Event {
            fd,
            readable: true,
            writable: true,
        }
    }
}

#[derive(Debug)]
pub enum InterestType {
    Read,
    Write,
}

pub struct Reactor {
    kqueue_fd: RawFd,
    notifier: (UnixStream, UnixStream),
    readable: HashMap<usize, Vec<Waker>>, //  change String to Vec<Waker>
    writable: HashMap<usize, Vec<Waker>>, //  change String to Vec<Waker>
}

impl Reactor {
    /// Create a reactor instance
    pub fn new() -> std::io::Result<Self> {
        let kq = unsafe { libc::kqueue() };
        if kq < 0 {
            return Err(std::io::Error::last_os_error());
        }
        let (reader, writer) = UnixStream::pair()?;
        reader.set_nonblocking(true)?;
        writer.set_nonblocking(true)?;
        let reactor = Reactor {
            kqueue_fd: kq.as_raw_fd(),
            notifier: (reader, writer),
            readable: HashMap::new(),
            writable: HashMap::new(),
        };

        reactor.modify(
            reactor.notifier.0.as_raw_fd(),
            Event::readable(reactor.notifier.0.as_raw_fd().try_into().unwrap()),
        )?;
        Ok(reactor)
    }

    /// Function to determine what interests this source has
    fn get_interest(&self, source: usize) -> Event {
        match (
            self.readable.contains_key(&source),
            self.writable.contains_key(&source),
        ) {
            (false, false) => Event::none(source),
            (true, false) => Event::readable(source),
            (false, true) => Event::writable(source),
            (true, true) => Event::all(source),
        }
    }

    /// Function to register interest for a specific source
    pub fn register_interest(
        &mut self,
        source: i32,
        interest: InterestType,
        context: &mut Context,
    ) {
        debug!(
            "Registering interest for source: {} with interest {:?}",
            source, interest
        );
        match interest {
            InterestType::Read => {
                self.readable
                    .entry(source as usize)
                    .and_modify(|v| v.push(context.waker().clone()))
                    .or_insert(vec![context.waker().clone()]);
                self.modify(source, Event::readable(source as usize))
                    .unwrap();
            }
            InterestType::Write => {
                self.writable
                    .entry(source as usize)
                    .and_modify(|v| v.push(context.waker().clone()))
                    .or_insert(vec![context.waker().clone()]);
                self.modify(source, Event::writable(source as usize))
                    .unwrap();
            }
        }
    }

    pub fn get_wakers(&mut self, events: Vec<Event>) -> Vec<Waker> {
        let mut wakers = Vec::new();
        for event in events {
            if event.readable {
                if let Some(readable_wakers) = self.readable.remove(&event.fd) {
                    wakers.extend(readable_wakers);
                }
            } else if event.writable {
                if let Some(writable_wakers) = self.writable.remove(&event.fd) {
                    wakers.extend(writable_wakers);
                }
            }
        }
        wakers
    }

    pub fn waiting_on_events(&self) -> bool {
        if self.readable.is_empty() && self.writable.is_empty() {
            return false;
        }
        true
    }

    /// Function to accept the source to register an interest in and the type of interest
    pub fn add(&mut self, source: RawFd) -> std::io::Result<()> {
        self.modify(source, self.get_interest(source as usize))
    }

    /// A helper notify method to unblock the scheduler
    fn notify(&mut self) -> std::io::Result<usize> {
        self.notifier.1.write(&[1])
    }

    /// The function that removes interest for a file descriptor with the actual underlying syscall
    pub fn remove(&self, fd: RawFd, ev: Event) -> std::io::Result<()> {
        let mut changelist = Vec::new();
        if ev.readable {
            changelist.push(kevent {
                ident: fd as _,
                filter: EVFILT_READ,
                flags: EV_DELETE,
                fflags: 0,
                data: 0,
                udata: ev.fd as *mut c_void,
            });
        }
        if ev.writable {
            changelist.push(kevent {
                ident: fd as _,
                filter: EVFILT_READ,
                flags: EV_DELETE,
                fflags: 0,
                data: 0,
                udata: ev.fd as *mut c_void,
            });
        }

        if changelist.is_empty() {
            return Ok(());
        }

        let result = unsafe {
            kevent(
                self.kqueue_fd,
                changelist.as_mut_ptr(),
                changelist.len() as i32,
                std::ptr::null_mut(),
                0,
                std::ptr::null(),
            )
        };
        if result < 0 {
            return Err(std::io::Error::last_os_error());
        }
        Ok(())
    }

    /// The function that registers interest with the actual underlying syscall
    fn modify(&self, fd: RawFd, ev: Event) -> std::io::Result<()> {
        let mut changelist = Vec::new();
        if ev.readable {
            changelist.push(kevent {
                ident: fd as _,
                filter: EVFILT_READ,
                flags: EV_ADD | EV_ONESHOT,
                fflags: 0,
                data: 0,
                udata: ev.fd as *mut c_void,
            });
        }

        if ev.writable {
            changelist.push(kevent {
                ident: fd as _,
                filter: EVFILT_WRITE,
                flags: EV_ADD | EV_ONESHOT,
                fflags: 0,
                data: 0,
                udata: ev.fd as *mut c_void,
            });
        }

        if changelist.is_empty() {
            return Ok(());
        }

        let result = unsafe {
            kevent(
                self.kqueue_fd,
                changelist.as_ptr(),
                changelist.len() as i32,
                std::ptr::null_mut(),
                0,
                std::ptr::null(),
            )
        };

        if result < 0 {
            return Err(std::io::Error::last_os_error());
        }
        Ok(())
    }

    /// Blocking poll function to get events
    pub fn poll(&mut self) -> std::io::Result<Vec<Event>> {
        let mut events: Vec<libc::kevent> = Vec::new();
        let result = unsafe {
            events.resize(1, std::mem::zeroed());
            kevent(
                self.kqueue_fd,
                std::ptr::null(),
                0,
                events.as_mut_ptr(),
                1,
                std::ptr::null(),
            )
        };

        if result < 0 {
            return Err(std::io::Error::last_os_error());
        }

        let mapped_events: std::io::Result<Vec<_>> = events
            .iter()
            .map(|event| {
                let ident = event.ident;
                let filter = event.filter;

                if ident == self.notifier.0.as_raw_fd().try_into().unwrap() {
                    let mut buf = [0; 8];
                    self.notifier.0.read(&mut buf)?;
                    self.modify(
                        self.notifier.0.as_raw_fd(),
                        Event::readable(self.notifier.0.as_raw_fd().try_into().unwrap()),
                    )?;
                }

                let event = Event {
                    fd: ident,
                    readable: filter == EVFILT_READ,
                    writable: filter == EVFILT_WRITE,
                };
                Ok(event)
            })
            .collect();
        mapped_events
    }
}
