use std::io::{Read, Write};
use std::net::TcpStream;
use std::thread;
use std::time::Instant;

fn send_request(id: usize) {
    let start = Instant::now();
    match TcpStream::connect("127.0.0.1:8000") {
        Ok(mut stream) => {
            println!("Thread {} connected to the server!", id);
            let msg = b"GET / HTTP/1.1\r\nHost: localhost:8000\r\n\r\n";
            stream.write_all(msg).unwrap();

            let mut buffer = [0; 1024];
            stream.read(&mut buffer).unwrap();

            let duration = start.elapsed();
            println!("Thread {} received response in {:?}", id, duration);
        }
        Err(e) => {
            println!("Thread {} failed to connect: {}", id, e);
        }
    }
}

fn main() {
    let num_threads = 5;
    let mut handles = vec![];

    let start = Instant::now();

    for i in 0..num_threads {
        let handle = thread::spawn(move || {
            send_request(i);
        });
        handles.push(handle);
    }

    for handle in handles {
        handle.join().unwrap();
    }

    let duration = start.elapsed();
    println!("All threads completed in {:?}", duration);
}
