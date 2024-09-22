use std::{collections::HashMap, ffi::c_void, os::fd::RawFd};

use libc::{kevent, EVFILT_READ, EVFILT_WRITE, EV_ADD, EV_DELETE, EV_ONESHOT};

pub struct Event {
    pub fd: usize,
    pub readable: bool,
    pub writable: bool,
}

impl Event {
    fn readable(fd: usize) -> Event {
        Event {
            fd,
            readable: true,
            writable: false,
        }
    }

    fn writable(fd: usize) -> Event {
        Event {
            fd,
            readable: false,
            writable: true,
        }
    }
}

enum InterestType {
    Read,
    Write,
}

struct Reactor {
    //  file descriptor for kqueue
    //  readable hashmap of raw fd -> waker mapping
    //  writable hashmap of raw fd -> waker mapping
    //  reference to the actual poller (use polling crate)
}

impl Reactor {
    /// Function to accept the source to register an interest in and the type of interest
    fn add() {}

    /// Blocking poll function to get events
    fn poll() {}

    /// A helper notify method to unblock the scheduler
    fn notify() {}

    /// The function that registers interest with the actual underlying syscall
    fn modify(&self, fd: RawFd, ev: Event) -> std::io::Result<()> {
        let read_flags = if ev.readable {
            EV_ADD | EV_ONESHOT
        } else {
            EV_DELETE
        };
        let write_flags = if ev.writable {
            EV_ADD | EV_ONESHOT
        } else {
            EV_DELETE
        };

        let changelist = [
            kevent {
                ident: fd as _,
                filter: EVFILT_READ,
                flags: read_flags,
                fflags: 0,
                data: 0,
                udata: ev.fd as *mut c_void,
            },
            kevent {
                ident: fd as _,
                filter: EVFILT_WRITE,
                flags: write_flags,
                fflags: 0,
                data: 0,
                udata: ev.fd as *mut c_void,
            },
        ];

        // unsafe { kevent(kq, changelist, nchanges, eventlist, nevents, timeout) };
        Ok(())
    }
}
