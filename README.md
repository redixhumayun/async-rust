## Understanding Async Rust
This is an educational project built to understand async runtimes in Rust

It contains 3 separate runtimes, each of which is slightly more complex than the last:
1. [The first](https://github.com/redixhumayun/async-rust/tree/main/src/futures_executors) is a very simple runtime which shows an executor which polls futures to completion.
2. [The second](https://github.com/redixhumayun/async-rust/tree/main/src/event_loop_reactors) is an event loop combined with a reactor.
3. [The third](https://github.com/redixhumayun/async-rust/tree/main/src/async_runtime) is a simple scheduler combined with a reactor, which can be used to poll futures to completion.

Have fun :)
