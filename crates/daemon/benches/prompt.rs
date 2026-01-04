use divan::{black_box, AllocProfiler, Bencher};
use starship_common::ShellContext;
use starship_daemon::{handle_client, handle_request};
use std::{
    io::{BufRead, BufReader, Write},
    os::unix::net::UnixStream,
    path::PathBuf,
};

#[global_allocator]
static ALLOC: AllocProfiler = AllocProfiler::system();

fn main() {
    divan::main();
}

fn test_context() -> ShellContext {
    ShellContext {
        pwd: Some(PathBuf::from("/Users/test/projects/starship")),
        user: Some("testuser".into()),
    }
}

/// Benchmark just the request handling logic
#[divan::bench]
fn handle_request_only(bencher: Bencher) {
    let json = serde_json::to_string(&test_context()).unwrap();

    bencher.bench(|| {
        let context: ShellContext = serde_json::from_str(black_box(&json)).unwrap();
        let prompt = handle_request(context);
        serde_json::to_string(&prompt).unwrap()
    });
}

/// Benchmark full client handling over `UnixStream`
#[divan::bench]
fn full_roundtrip(bencher: Bencher) {
    let request = serde_json::to_string(&test_context()).unwrap();

    bencher.bench(|| {
        let (mut client, server) = UnixStream::pair().unwrap();

        std::thread::spawn(move || handle_client(server));

        client.write_all(request.as_bytes()).unwrap();
        client.shutdown(std::net::Shutdown::Write).unwrap();

        let mut response = String::new();
        BufReader::new(&client).read_line(&mut response).unwrap();
        black_box(response)
    });
}
