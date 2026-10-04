use config::BenchConfig;
use divan::{Bencher, black_box};
use starship_common::ShellContext;
use starship_daemon::handle_client;
use starship_runtime::ConfigLoader;
use starship_runtime::plugin::PluginProcess;
use starship_runtime::plugin::test_helpers::{PluginFixture, plugin_binary};
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

const PLUGIN_EXPR: &str = r#"compact(green("test:", test.home), ctx.pwd, "❯")"#;

const ALL_CONFIGS: [BenchConfig; 3] = [MINIMAL_CONFIG, WITH_MODULES_CONFIG, COMPACT_CONFIG];

fn main() {
    divan::main();
}

fn context() -> ShellContext {
    ShellContext {
        pwd: Some(PathBuf::from("/Users/test/projects/starship")),
        user: Some("testuser".into()),
        ..ShellContext::default()
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
    let ctx = context();
    let func = loader.load(&ctx).unwrap();
    let output: starship_runtime::Config = func.call(()).unwrap();
    black_box(output.format.to_string());
}

#[divan::bench(args = ALL_CONFIGS)]
fn cached_config(bencher: Bencher, config: &BenchConfig) {
    let mut loader = ConfigLoader::from_source(config.source).unwrap();
    bencher.bench_local(|| {
        let ctx = context();
        let func = loader.load(&ctx).unwrap();
        let output: starship_runtime::Config = func.call(()).unwrap();
        black_box(output.format.to_string())
    });
}

// --- Plugin benchmarks ---

#[divan::bench]
fn plugin_load() {
    let binary = plugin_binary("starship-plugin-test-harness");
    black_box(PluginProcess::spawn(&binary, None).unwrap());
}

#[divan::bench]
fn plugin_call_method(bencher: Bencher) {
    let mut fixture = PluginFixture::test_harness();
    std::fs::write(fixture.dir.join(".starship-test-marker"), "").unwrap();
    bencher.bench_local(|| {
        black_box(fixture.get("home"));
    });
}

#[divan::bench]
fn config_with_plugins(bencher: Bencher) {
    let mut fixture = PluginFixture::test_harness();
    std::fs::write(fixture.dir.join(".starship-test-marker"), "").unwrap();
    bencher.bench_local(|| {
        black_box(fixture.render(PLUGIN_EXPR));
    });
}
