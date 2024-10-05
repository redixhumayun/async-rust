## Understanding Async Rust
This is an educational project built to understand async runtimes in Rust

It contains 3 separate runtimes, each of which is slightly more complex than the last:
1. [The first](https://github.com/redixhumayun/async-rust/tree/main/src/futures_executors) is a very simple runtime which shows an executor which polls futures to completion.
2. [The second](https://github.com/redixhumayun/async-rust/tree/main/src/event_loop_reactors) is an event loop combined with a reactor.
3. [The third](https://github.com/redixhumayun/async-rust/tree/main/src/async_runtime) is a simple scheduler combined with a reactor, which can be used to poll futures to completion.

To run any runtime, simply go into the folder for the runtime and just do `cargo build` followed by `cargo run`. Repos 2 & 3 will start up a TCP server to which you can then send requests.

There is a test client application located in the folders for 2 & 3 used to simulate network requests. You can run `rustc ./src/async_runtime/test_client.rs` and this will produce a binary called `test_client` in the root directory. Once you have the server running, do `./test_client` in a separate tab to send some requests.

Have fun :)
