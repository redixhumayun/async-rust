use crate::EventHandler;

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
