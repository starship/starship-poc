//! Plugin process costs, measured against the test plugin's real binary.

use std::path::PathBuf;

use divan::{Bencher, black_box};
use starship_runtime::plugin::PluginProcess;
use starship_runtime::plugin::test_helpers::{PluginFixture, render_context};

/// The test plugin's binary. CI builds the benches on one machine and runs
/// them on another with a different checkout path, so `STARSHIP_TEST_PLUGIN`
/// can point at where the binary actually is.
fn binary() -> PathBuf {
    #[expect(clippy::disallowed_methods, reason = "bench setup, not plugin code")]
    let configured = std::env::var_os("STARSHIP_TEST_PLUGIN");
    configured.map_or_else(
        || PathBuf::from(env!("CARGO_BIN_EXE_starship-plugin-test-harness")),
        PathBuf::from,
    )
}

fn main() {
    divan::main();
}

/// Starting the process and reading its manifest.
#[divan::bench]
fn plugin_load() {
    black_box(PluginProcess::spawn(&binary(), None).unwrap());
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
    let mut plugin = PluginProcess::spawn(&binary(), None).unwrap();
    plugin.begin_render(1, &context);
    bencher.bench_local(|| {
        black_box(plugin.call_method(1, "home"));
    });
}

/// A full render of a config reading three fields from one plugin.
#[divan::bench]
fn render_with_plugin_reads(bencher: Bencher) {
    let fixture = PluginFixture::from_binary(binary());
    std::fs::write(fixture.dir.join(".starship-test-marker"), "").unwrap();
    let mut loader =
        fixture.loader(r#"compact(green("test:", test.home), test.user, test.dir, "❯")"#);
    let context = fixture.context();
    bencher.bench_local(|| {
        black_box(loader.render(&context).unwrap().format.to_string());
    });
}
