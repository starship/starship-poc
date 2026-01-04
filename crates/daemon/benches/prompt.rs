use divan::{black_box, AllocProfiler, Bencher};
use starship_common::ShellContext;
use starship_daemon::{config::ConfigLoader, handle_client};
use std::{
    io::{BufRead, BufReader, Write},
    os::unix::net::UnixStream,
    path::PathBuf,
    sync::{Arc, Mutex},
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

#[divan::bench]
fn full_roundtrip(bencher: Bencher) {
    let request = format!("{}\n", serde_json::to_string(&test_context()).unwrap());
    let loader = Arc::new(Mutex::new(ConfigLoader::new().unwrap()));

    bencher.bench(|| {
        let (mut client, server) = UnixStream::pair().unwrap();
        let loader = Arc::clone(&loader);

        std::thread::spawn(move || {
            let mut loader = loader.lock().unwrap();
            handle_client(server, &mut loader).unwrap();
        });

        client.write_all(request.as_bytes()).unwrap();
        client.shutdown(std::net::Shutdown::Write).unwrap();

        let mut response = String::new();
        BufReader::new(&client).read_line(&mut response).unwrap();
        black_box(response)
    });
}
