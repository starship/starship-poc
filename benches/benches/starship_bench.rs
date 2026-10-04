use config::BenchConfig;
use divan::{Bencher, black_box};
use starship_common::RenderContext;
use starship_daemon::handle_client;
use starship_runtime::ConfigLoader;
use std::{os::unix::net::UnixStream, path::PathBuf};

mod config;

const MINIMAL_CONFIG: BenchConfig = BenchConfig {
    name: "Minimal",
    source: r#"
        return "$ "
    "#,
};

const WITH_MODULES_CONFIG: BenchConfig = BenchConfig {
    name: "With Modules",
    source: r#"
        return ctx.pwd .. " " .. ctx.user .. " $ "
    "#,
};

const COMPACT_CONFIG: BenchConfig = BenchConfig {
    name: "Compact",
    source: r#"
        return compact(green("node:", nil), ctx.pwd, "❯")
    "#,
};

const ALL_CONFIGS: [BenchConfig; 3] = [MINIMAL_CONFIG, WITH_MODULES_CONFIG, COMPACT_CONFIG];

fn main() {
    divan::main();
}

fn context() -> RenderContext {
    RenderContext {
        pwd: PathBuf::from("/Users/test/projects/starship"),
        env: [("USER".to_string(), "testuser".to_string())].into(),
    }
}

// --- Socket-based benchmark (end-to-end with IPC) ---

#[divan::bench(args = [MINIMAL_CONFIG])]
fn socket_render(bencher: Bencher, config: &BenchConfig) {
    let mut loader = ConfigLoader::from_source(config.source).unwrap();
    bencher.bench_local(|| {
        let (client, server) = UnixStream::pair().unwrap();
        std::thread::scope(|s| {
            let handle = s.spawn(|| starship::run(client, &context()).unwrap());
            handle_client(server, &mut loader).unwrap();
            black_box(handle.join().unwrap())
        })
    });
}

// --- Daemonless benchmarks (runtime directly, no IPC) ---

#[divan::bench(args = ALL_CONFIGS)]
fn cold_start(config: &BenchConfig) {
    let mut loader = ConfigLoader::from_source(config.source).unwrap();
    let output = loader.render(&context()).unwrap();
    black_box(output.format.to_string());
}

#[divan::bench(args = ALL_CONFIGS)]
fn cached_config(bencher: Bencher, config: &BenchConfig) {
    let mut loader = ConfigLoader::from_source(config.source).unwrap();
    bencher.bench_local(|| {
        let output = loader.render(&context()).unwrap();
        black_box(output.format.to_string())
    });
}
