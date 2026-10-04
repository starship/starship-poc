use anyhow::{Context, Result};
use starship_common::ShellContext;
use starship_runtime::{Config, ConfigLoader};
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
        let output: Config =
            tracing::info_span!("lua_eval").in_scope(|| config_function.call(()))?;

        let writer = reader.get_mut();
        tracing::info_span!("serialize")
            .in_scope(|| serde_json::to_writer(&mut *writer, &output.format))?;
        writer.write_all(b"\n")?;
        writer.flush()?;

        line.clear();
    }

    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use starship_runtime::plugin::test_helpers::PluginFixture;
    use std::os::unix::net::UnixStream;

    #[test]
    fn client_receives_styled_prompt_over_socket() {
        let dir = tempfile::tempdir().expect("tempdir");
        let pwd = dir.path().to_str().expect("tempdir path utf8");
        let mut loader = ConfigLoader::from_source(r#"return green(ctx.pwd .. " $ ")"#).unwrap();
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

    #[test]
    fn daemon_serves_prompt_with_plugin_data() {
        let mut plugin = PluginFixture::test_harness();
        std::fs::write(plugin.dir.join(".starship-test-marker"), "").unwrap();
        let result = plugin.render(r#"test.home or "none""#);
        assert_ne!(result, "");
        assert_ne!(result, "none");
    }

    #[test]
    fn plugin_method_returns_nil_when_inapplicable() {
        let mut plugin = PluginFixture::test_harness();
        let result = plugin.render(r#"test.home or "inapplicable""#);
        assert_eq!(result, "inapplicable");
    }
}
