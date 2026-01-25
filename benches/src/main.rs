use config::BenchConfig;
use divan::{Bencher, black_box};
use starship_common::ShellContext;
use starship_daemon::{config::ConfigLoader, handle_client};
use std::{os::unix::net::UnixStream, path::PathBuf};

mod config;

const CONFIGS: [BenchConfig; 2] = [
    BenchConfig {
        name: "Minimal",
        source: r#"
          return { format = "$ " }
        "#,
    },
    BenchConfig {
        name: "With Modules",
        source: r#"
          return { format = ctx.pwd .. " " .. ctx.user .. " $ " }
        "#,
    },
];

fn main() {
    divan::main();
}

fn context() -> ShellContext {
    ShellContext {
        pwd: Some(PathBuf::from("/Users/test/projects/starship")),
        user: Some("testuser".into()),
    }
}

#[divan::bench(args = CONFIGS, sample_count = 1000)]
fn cold_start_render(config: &BenchConfig) {
    let mut loader = ConfigLoader::from_source(config.source).unwrap();
    let (client, server) = UnixStream::pair().unwrap();
    std::thread::spawn(move || handle_client(server, &mut loader).unwrap());
    starship::run(client, &context()).unwrap();
}

#[divan::bench(args = CONFIGS, sample_count = 1000)]
fn cached_render(bencher: Bencher, config: &BenchConfig) {
    let mut loader = ConfigLoader::from_source(config.source).unwrap();
    bencher.bench_local(|| {
        let (client, server) = UnixStream::pair().unwrap();
        std::thread::scope(|s| {
            s.spawn(|| handle_client(server, &mut loader).unwrap());
            black_box(starship::run(client, &context()).unwrap())
        })
    });
}
