use std::{
    future::Future,
    sync::{
        mpsc::{sync_channel, Receiver, SyncSender},
        Arc, Mutex,
    },
    task::{Context, Poll, Waker},
    thread,
    time::Duration,
};

use futures::{
    future::{BoxFuture, FutureExt},
    task::{waker_ref, ArcWake},
};

struct SharedState {
    completed: bool,
    waker: Option<Waker>,
}

pub struct TimerFuture {
    shared_state: Arc<Mutex<SharedState>>,
}

impl Future for TimerFuture {
    type Output = ();
    fn poll(
        self: std::pin::Pin<&mut Self>,
        cx: &mut std::task::Context<'_>,
    ) -> std::task::Poll<Self::Output> {
        println!("polling the future");
        let mut shared_state = self.shared_state.lock().unwrap();
        if shared_state.completed {
            return Poll::Ready(());
        }
        println!("future is still pending");
        shared_state.waker = Some(cx.waker().clone());
        println!("registered the waker");
        return Poll::Pending;
    }
}

impl TimerFuture {
    fn new(duration: Duration) -> Self {
        let shared_state = Arc::new(Mutex::new(SharedState {
            completed: false,
            waker: None,
        }));
        let thread_shared_state = Arc::clone(&shared_state);
        thread::spawn(move || {
            thread::sleep(duration);
            let mut shared_state = thread_shared_state.lock().unwrap();
            shared_state.completed = true;
            println!("duration has passed, calling .wake()");
            if let Some(waker) = shared_state.waker.take() {
                waker.wake();
            }
        });
        TimerFuture { shared_state }
    }
}

struct Task {
    future: Mutex<BoxFuture<'static, ()>>,
    task_sender: SyncSender<Arc<Task>>,
}

impl ArcWake for Task {
    fn wake_by_ref(arc_self: &Arc<Self>) {
        println!("called arc wake for task, re-sending the task");
        let arc_clone = Arc::clone(&arc_self);
        arc_self.task_sender.send(arc_clone).unwrap();
    }
}

struct Spawner {
    task_sender: SyncSender<Arc<Task>>,
}

impl Spawner {
    fn spawn(&self, future: impl Future<Output = ()> + Send + 'static) {
        let box_future = future.boxed();
        let task = Arc::new(Task {
            future: Mutex::new(box_future),
            task_sender: self.task_sender.clone(),
        });
        self.task_sender.send(task).unwrap();
    }
}

struct Executor {
    task_queue: Receiver<Arc<Task>>,
}

impl Executor {
    fn run(&self) {
        println!("run");
        while let Ok(task) = self.task_queue.recv() {
            println!("received task");
            let mut fut = task.future.lock().unwrap();
            let waker = waker_ref(&task);
            let context = &mut Context::from_waker(&waker);
            if fut.as_mut().poll(context).is_ready() {
                println!("The future is done running");
            }
        }
    }

    fn executor_and_spawner() -> (Executor, Spawner) {
        let (sync_sender, receiver) = sync_channel(10000);
        let executor = Executor {
            task_queue: receiver,
        };
        let spawner = Spawner {
            task_sender: sync_sender,
        };
        (executor, spawner)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn spawn_tasks() {
        let (executor, spawner) = Executor::executor_and_spawner();
        spawner.spawn(async {
            println!("Hello");
            TimerFuture::new(Duration::from_secs(2)).await;
            println!("World");
        });
        drop(spawner);
        executor.run();
    }
}
