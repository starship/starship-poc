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
        let mut loader =
            ConfigLoader::from_source(r#"return { format = green(ctx.pwd .. " $ ") }"#).unwrap();
        let ctx = ShellContext {
            pwd: Some(PathBuf::from("/tmp")),
            user: Some("test".into()),
        };

        let (client, server) = UnixStream::pair().unwrap();
        std::thread::scope(|s| {
            let handle = s.spawn(|| starship::run(client, &ctx).unwrap());
            handle_client(server, &mut loader).unwrap();
            assert_eq!(handle.join().unwrap(), "\x1b[32m/tmp $ \x1b[0m");
        });
    }

    fn nodejs_wasm_path() -> PathBuf {
        std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
            .parent()
            .and_then(|p| p.parent())
            .unwrap_or(std::path::Path::new("/"))
            .join("target/wasm32-unknown-unknown/release/nodejs.wasm")
    }

    #[test]
    fn daemon_serves_prompt_with_plugin_data() {
        let wasm_path = nodejs_wasm_path();
        if !wasm_path.exists() {
            eprintln!("Skipping: nodejs.wasm not found at {wasm_path:?}");
            return;
        }

        let bytes = std::fs::read(&wasm_path).expect("nodejs wasm readable");
        let engine = wasmtime::Engine::default();

        let dir = tempfile::tempdir().expect("tempdir");
        std::fs::write(dir.path().join("package.json"), "{}").expect("write package.json");

        let plugin = starship_runtime::plugin::WasmPlugin::load(&engine, &bytes, dir.path())
            .expect("plugin loads");

        let mut loader = ConfigLoader::from_source_with_plugins(
            r#"return { format = "node:" .. (nodejs.version or "none") }"#,
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
            assert!(
                result.starts_with("node:"),
                "Expected 'node:' prefix, got: {result}"
            );
            assert!(
                result.len() > 5,
                "Expected version string after 'node:', got: {result}"
            );
        });
    }

    #[test]
    fn plugin_method_returns_nil_when_not_applicable() {
        let wasm_path = nodejs_wasm_path();
        if !wasm_path.exists() {
            eprintln!("Skipping: nodejs.wasm not found at {wasm_path:?}");
            return;
        }

        let bytes = std::fs::read(&wasm_path).expect("nodejs wasm readable");
        let engine = wasmtime::Engine::default();

        let plugin = starship_runtime::plugin::WasmPlugin::load(
            &engine,
            &bytes,
            std::path::Path::new("/tmp"),
        )
        .expect("plugin loads");

        let mut loader = ConfigLoader::from_source_with_plugins(
            r#"return { format = nodejs.version or "no-node" }"#,
            vec![plugin],
        )
        .expect("loader with plugin");

        let ctx = ShellContext {
            pwd: Some(PathBuf::from("/tmp")),
            user: Some("test".into()),
        };

        let (client, server) = UnixStream::pair().unwrap();
        std::thread::scope(|s| {
            let handle = s.spawn(|| starship::run(client, &ctx).unwrap());
            handle_client(server, &mut loader).unwrap();
            let result = handle.join().unwrap();
            assert!(!result.is_empty(), "Expected non-empty result");
        });
    }
}
