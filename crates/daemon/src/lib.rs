// Re-export runtime for backward compatibility
pub use starship_runtime::*;

use anyhow::{Context, Result};
use starship_common::ShellContext;
use std::io::{BufRead, BufReader, Read, Write};
use tracing::instrument;

/// Handles a client connection, loading the config and responding with the prompt.
#[instrument(skip_all)]
pub fn handle_client<S: Read + Write>(stream: S, loader: &mut ConfigLoader) -> Result<()> {
    let mut reader = BufReader::with_capacity(512, stream);
    let mut line = String::new();

    while reader.read_line(&mut line)? > 0 {
        if line.trim().is_empty() {
            line.clear();
            continue;
        }

        let context: ShellContext =
            serde_json::from_str(&line).context("Failed to parse request")?;
        let config_function = loader.load(&context)?;
        let output: Config = config_function.call(())?;

        let writer = reader.get_mut();
        serde_json::to_writer(&mut *writer, &output.format)?;
        writer.write_all(b"\n")?;
        writer.flush()?;

        line.clear();
    }

    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::os::unix::net::UnixStream;
    use std::path::PathBuf;

    #[test]
    fn client_receives_styled_prompt_over_socket() {
        let dir = tempfile::tempdir().expect("tempdir");
        let pwd = dir.path().to_str().expect("tempdir path utf8");
        let mut loader =
            ConfigLoader::from_source(r#"return { format = green(ctx.pwd .. " $ ") }"#).unwrap();
        let ctx = ShellContext {
            pwd: Some(dir.path().to_path_buf()),
            user: Some("test".into()),
        };

        let (client, server) = UnixStream::pair().unwrap();
        std::thread::scope(|s| {
            let handle = s.spawn(|| starship::run(client, &ctx).unwrap());
            handle_client(server, &mut loader).unwrap();
            assert_eq!(handle.join().unwrap(), format!("\x1b[32m{pwd} $ \x1b[0m"));
        });
    }

    fn test_harness_wasm_path() -> PathBuf {
        std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
            .parent()
            .and_then(|p| p.parent())
            .unwrap_or(std::path::Path::new("/"))
            .join("target/wasm32-unknown-unknown/release/starship_plugin_test_harness.wasm")
    }

    #[test]
    fn daemon_serves_prompt_with_plugin_data() {
        let bytes =
            std::fs::read(test_harness_wasm_path()).expect("test-harness.wasm should exist");
        let engine = wasmtime::Engine::default();

        let dir = tempfile::tempdir().expect("tempdir");
        std::fs::write(dir.path().join(".starship-test-marker"), "").unwrap();

        let plugin = starship_runtime::plugin::WasmPlugin::load(&engine, &bytes, dir.path())
            .expect("plugin loads");

        let mut loader = ConfigLoader::from_source_with_plugins(
            r#"return { format = test.home or "none" }"#,
            vec![plugin],
        )
        .expect("loader with plugin");

        let ctx = ShellContext {
            pwd: Some(dir.path().to_path_buf()),
            user: Some("test".into()),
        };

        let (client, server) = UnixStream::pair().unwrap();
        std::thread::scope(|s| {
            let handle = s.spawn(|| starship::run(client, &ctx).unwrap());
            handle_client(server, &mut loader).unwrap();
            let result = handle.join().unwrap();
            assert!(!result.is_empty(), "expected HOME value, got empty");
            assert_ne!(result, "none", "expected HOME value, got fallback");
        });
    }

    #[test]
    fn plugin_method_returns_nil_when_inactive() {
        let bytes =
            std::fs::read(test_harness_wasm_path()).expect("test-harness.wasm should exist");
        let engine = wasmtime::Engine::default();
        let dir = tempfile::tempdir().expect("tempdir");

        let plugin = starship_runtime::plugin::WasmPlugin::load(&engine, &bytes, dir.path())
            .expect("plugin loads");

        let mut loader = ConfigLoader::from_source_with_plugins(
            r#"return { format = test.home or "inactive" }"#,
            vec![plugin],
        )
        .expect("loader with plugin");

        let ctx = ShellContext {
            pwd: Some(dir.path().to_path_buf()),
            user: Some("test".into()),
        };

        let (client, server) = UnixStream::pair().unwrap();
        std::thread::scope(|s| {
            let handle = s.spawn(|| starship::run(client, &ctx).unwrap());
            handle_client(server, &mut loader).unwrap();
            assert_eq!(handle.join().unwrap(), "inactive");
        });
    }
}
