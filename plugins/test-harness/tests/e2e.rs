//! Process-level behavior, checked against the test plugin's real binary
//! through the daemon's plugin client.

use std::fs;
use std::path::Path;
use std::process::{Command, Stdio};
use std::time::{Duration, Instant};

use serde_json::Value;
use starship_runtime::plugin::test_helpers::{PluginFixture, render_context};
use starship_runtime::plugin::{PluginKind, PluginProcess, load_plugins};

const BINARY: &str = env!("CARGO_BIN_EXE_starship-plugin-test-harness");

fn fixture() -> PluginFixture {
    PluginFixture::from_binary(BINARY)
}

#[test]
fn binary_reports_its_manifest() {
    let plugin = fixture();
    assert_eq!(plugin.name(), "test");
    assert_eq!(plugin.kind(), PluginKind::General);
    assert_eq!(plugin.shadows(), Vec::<String>::new());
}

#[test]
fn binary_answers_within_a_render() {
    let mut plugin = fixture();
    assert!(!plugin.is_applicable());
    fs::write(plugin.dir.join(".starship-test-marker"), "").unwrap();
    assert!(plugin.is_applicable());
    assert_eq!(plugin.get("user").as_deref(), Some("test"));
    assert!(plugin.get("does_not_exist").is_none());
}

#[test]
fn binary_keeps_overlapping_renders_apart() {
    let first = tempfile::tempdir().unwrap();
    let second = tempfile::tempdir().unwrap();
    let mut plugin = fixture();
    let process = plugin.process();
    process.begin_render(1, &render_context(first.path()));
    process.begin_render(2, &render_context(second.path()));

    let pwd = |process: &mut PluginProcess, render_id| {
        let output = process.call_method(render_id, "pwd");
        fs::canonicalize(output.as_str().expect("pwd output")).unwrap()
    };
    assert_eq!(pwd(process, 2), fs::canonicalize(second.path()).unwrap());
    assert_eq!(pwd(process, 1), fs::canonicalize(first.path()).unwrap());

    process.end_render(1);
    assert_eq!(process.call_method(1, "pwd"), Value::Null);
}

#[test]
fn crashed_binary_is_inapplicable_and_returns_nil() {
    let mut plugin = fixture();
    fs::write(plugin.dir.join(".starship-test-marker"), "").unwrap();
    let context = plugin.context();
    let process = plugin.process();
    assert!(process.begin_render(1, &context).applicable);

    process.kill();
    assert!(!process.begin_render(2, &context).applicable);
    assert_eq!(process.call_method(2, "user"), Value::Null);
}

#[test]
fn binary_renders_through_a_config() {
    let plugin = fixture();
    fs::write(plugin.dir.join(".starship-test-marker"), "").unwrap();
    assert_eq!(plugin.render(r#"compact(test.user, "❯")"#), "test ❯");
}

#[test]
fn binary_exits_when_stdin_closes() {
    #[expect(clippy::disallowed_methods, reason = "starts the plugin binary itself")]
    let mut child = Command::new(BINARY)
        .stdin(Stdio::piped())
        .stdout(Stdio::null())
        .spawn()
        .unwrap();
    drop(child.stdin.take());

    let deadline = Instant::now() + Duration::from_secs(5);
    let status = loop {
        if let Some(status) = child.try_wait().unwrap() {
            break status;
        }
        assert!(Instant::now() < deadline, "plugin didn't exit on EOF");
        std::thread::sleep(Duration::from_millis(10));
    };
    assert!(status.success());
}

#[cfg(unix)]
#[test]
fn discovery_only_starts_plugin_executables() {
    use std::os::unix::fs::PermissionsExt;

    let plugin_dir = tempfile::tempdir().unwrap();
    let name = Path::new(BINARY).file_name().unwrap();
    std::os::unix::fs::symlink(BINARY, plugin_dir.path().join(name)).unwrap();
    // An executable that isn't a plugin, like `starship-daemon` sitting in
    // `target/debug`. It leaves a marker if anything runs it.
    let marker = plugin_dir.path().join("ran");
    let other = plugin_dir.path().join("starship-daemon");
    fs::write(&other, format!("#!/bin/sh\ntouch '{}'\n", marker.display())).unwrap();
    fs::set_permissions(&other, fs::Permissions::from_mode(0o755)).unwrap();

    let plugins = load_plugins(plugin_dir.path(), None);
    let names: Vec<_> = plugins.iter().map(PluginProcess::name).collect();
    assert_eq!(names, ["test"]);
    assert!(!marker.exists());
}
