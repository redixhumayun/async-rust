#![allow(dead_code)]
use std::{
    rc::Rc,
    sync::mpsc::Sender,
    task::{RawWaker, RawWakerVTable, Waker},
};

use super::task_queue::Task;

pub struct MyWaker {
    task: Rc<Task>,
    sender: Sender<Rc<Task>>,
}

impl MyWaker {
    const VTABLE: RawWakerVTable =
        RawWakerVTable::new(Self::clone, Self::wake, Self::wake_by_ref, Self::drop);

    pub fn new(task: Rc<Task>, sender: Sender<Rc<Task>>) -> Waker {
        let pointer = Rc::into_raw(Rc::new(MyWaker { task, sender })) as *const ();
        let vtable = &MyWaker::VTABLE;
        unsafe { Waker::from_raw(RawWaker::new(pointer, vtable)) }
    }

    unsafe fn clone(ptr: *const ()) -> RawWaker {
        let waker = std::mem::ManuallyDrop::new(Rc::from_raw(ptr as *const MyWaker));
        let cloned_waker = Rc::clone(&waker);
        let raw_pointer = Rc::into_raw(cloned_waker);
        RawWaker::new(raw_pointer as *const (), &Self::VTABLE)
    }

    unsafe fn wake(ptr: *const ()) {
        let waker = Rc::from_raw(ptr as *const MyWaker);
        waker.sender.send(Rc::clone(&waker.task)).unwrap();
    }

    unsafe fn wake_by_ref(ptr: *const ()) {
        let waker = &*(ptr as *const MyWaker);
        waker.sender.send(Rc::clone(&waker.task)).unwrap();
    }

    unsafe fn drop(ptr: *const ()) {
        drop(Rc::from_raw(ptr as *const MyWaker));
    }
}
