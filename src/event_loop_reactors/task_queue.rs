use std::fmt::Display;

use super::EventHandler;

pub struct RegistrationTask {
    pub fd: usize,
    pub reference: Box<dyn EventHandler>,
}

pub struct UnregistrationTask {
    pub fd: usize,
}

pub struct ScheduledTask {
    pub fd: usize,
}

pub enum Task {
    RegistrationTask(RegistrationTask),
    UnregistrationTask(UnregistrationTask),
    ScheduledTask(ScheduledTask),
}

impl Display for Task {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Task::RegistrationTask(task) => write!(f, "RegistrationTask: fd {}", task.fd),
            Task::UnregistrationTask(task) => write!(f, "UnregistrationTask: fd {}", task.fd),
            Task::ScheduledTask(task) => write!(f, "ScheduledTask: fd {}", task.fd),
        }
    }
}

pub struct TaskQueue {
    pub queue: Vec<Task>,
}

impl TaskQueue {
    pub fn new() -> Self {
        Self { queue: Vec::new() }
    }

    pub fn add_task(&mut self, task: Task) {
        self.queue.push(task);
    }
}
