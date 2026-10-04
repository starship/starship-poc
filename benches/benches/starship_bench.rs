use config::BenchConfig;
use divan::{Bencher, black_box};
use starship_common::RenderContext;
use starship_daemon::handle_client;
use starship_runtime::ConfigLoader;
use starship_runtime::plugin::PluginProcess;
use starship_runtime::plugin::test_helpers::{PluginFixture, plugin_binary, render_context};
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

const PLUGIN_EXPR: &str = r#"compact(green("test:", test.home), test.user, test.dir, "❯")"#;

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

// --- Plugin benchmarks ---

#[divan::bench]
fn plugin_load() {
    let binary = plugin_binary("starship-plugin-test-harness");
    black_box(PluginProcess::spawn(&binary, None).unwrap());
}

/// One field read within a begun render. With `full_env`, the render carries
/// the bench process's environment; otherwise an empty one.
#[divan::bench(args = [false, true])]
fn plugin_call_method(bencher: Bencher, full_env: bool) {
    let dir = tempfile::tempdir().unwrap();
    let mut context = render_context(dir.path());
    if !full_env {
        context.env.clear();
    }
    let mut plugin =
        PluginProcess::spawn(&plugin_binary("starship-plugin-test-harness"), None).unwrap();
    plugin.begin_render(1, &context);
    bencher.bench_local(|| {
        black_box(plugin.call_method(1, "home"));
    });
}

/// A full render of a config reading three fields from one plugin.
#[divan::bench]
fn render_with_plugin_reads(bencher: Bencher) {
    let fixture = PluginFixture::test_harness();
    std::fs::write(fixture.dir.join(".starship-test-marker"), "").unwrap();
    let mut loader = fixture.loader(PLUGIN_EXPR);
    let context = fixture.context();
    bencher.bench_local(|| {
        black_box(loader.render(&context).unwrap().format.to_string());
    });
}
