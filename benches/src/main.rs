use config::BenchConfig;
use divan::{black_box, Bencher};
use starship_common::{render_prompt, ShellContext};
use starship_daemon::handle_client;
use starship_runtime::ConfigLoader;
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

// --- Socket-based benchmark (end-to-end with IPC) ---

#[divan::bench(args = [&CONFIGS[0]])]
fn socket_render(bencher: Bencher, config: &BenchConfig) {
    let mut loader = ConfigLoader::from_source(config.source).unwrap();
    bencher.bench_local(|| {
        let (client, server) = UnixStream::pair().unwrap();
        std::thread::scope(|s| {
            s.spawn(|| handle_client(server, &mut loader).unwrap());
            black_box(starship::run(client, &context()).unwrap())
        })
    });
}

// --- Daemonless benchmarks (runtime directly, no IPC) ---

#[divan::bench(args = CONFIGS)]
fn cold_start(config: &BenchConfig) {
    let mut loader = ConfigLoader::from_source(config.source).unwrap();
    let ctx = context();
    let func = loader.load(&ctx).unwrap();
    let output: starship_runtime::Config = func.call(()).unwrap();
    black_box(render_prompt(&output.format));
}

#[divan::bench(args = CONFIGS)]
fn cached_config(bencher: Bencher, config: &BenchConfig) {
    let mut loader = ConfigLoader::from_source(config.source).unwrap();
    bencher.bench_local(|| {
        let ctx = context();
        let func = loader.load(&ctx).unwrap();
        let output: starship_runtime::Config = func.call(()).unwrap();
        black_box(render_prompt(&output.format))
    });
}
