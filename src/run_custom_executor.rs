use std::time::Duration;
use timer_future::{Executor, TimerFuture};

fn run_custom_executor() {
    let (executor, spawner) = Executor::executor_and_spawner();
    spawner.spawn(async {
        println!("Hello");
        TimerFuture::new(Duration::from_secs(2)).await;
        println!("World");
    });
    spawner.spawn(async {
        println!("intermediate result");
    });
    drop(spawner);
    executor.run();
}

fn main() {
    run_custom_executor();
}
