use anyhow::{Context, Result};
use starship_common::RenderContext;
use starship_runtime::ConfigLoader;
use std::io::{BufRead, BufReader, Read, Write};
use tracing::instrument;

/// Handles a client connection, rendering the config and responding with the prompt.
#[instrument(skip_all)]
pub fn handle_client<S: Read + Write>(stream: S, loader: &mut ConfigLoader) -> Result<()> {
    let mut reader = BufReader::with_capacity(512, stream);
    let mut line = String::new();

    while reader.read_line(&mut line)? > 0 {
        if line.trim().is_empty() {
            line.clear();
            continue;
        }

        let context: RenderContext =
            serde_json::from_str(&line).context("Failed to parse request")?;
        let output = loader.render(&context)?;

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
        let ctx = RenderContext {
            pwd: dir.path().to_path_buf(),
            env: [("USER".to_string(), "test".to_string())].into(),
        };

        let (client, server) = UnixStream::pair().unwrap();
        std::thread::scope(|s| {
            let handle = s.spawn(|| starship::run(client, &ctx).unwrap());
            handle_client(server, &mut loader).unwrap();
            assert_eq!(handle.join().unwrap(), format!("\x1b[32m{pwd} $ \x1b[0m"));
        });
    }

    #[test]
    fn daemon_answers_raw_json_requests_like_the_readme_nc_example() {
        use starship_common::styled::StyledContent;
        use std::io::Read;

        let source = r#"return ctx.pwd .. " " .. (ctx.user or "nobody")"#;
        let mut loader = ConfigLoader::from_source(source).unwrap();
        let (mut client, server) = UnixStream::pair().unwrap();
        client
            .write_all(b"{\"pwd\":\"/tmp\",\"env\":{\"USER\":\"u\"}}\n{\"pwd\":\"/tmp\"}\n")
            .unwrap();
        client.shutdown(std::net::Shutdown::Write).unwrap();
        handle_client(server, &mut loader).unwrap();

        let mut output = String::new();
        client.read_to_string(&mut output).unwrap();
        let prompts: Vec<String> = output
            .lines()
            .map(|line| {
                serde_json::from_str::<StyledContent>(line)
                    .unwrap()
                    .to_string()
            })
            .collect();
        assert_eq!(prompts, ["/tmp u", "/tmp nobody"]);
    }

    #[test]
    fn daemon_serves_prompt_with_plugin_data() {
        let plugin = PluginFixture::test_plugin();
        std::fs::write(plugin.dir.join(".starship-test-marker"), "").unwrap();
        let result = plugin.render(r#"test.home or "none""#);
        assert_ne!(result, "");
        assert_ne!(result, "none");
    }

    #[test]
    fn plugin_method_returns_nil_when_inapplicable() {
        let plugin = PluginFixture::test_plugin();
        let result = plugin.render(r#"test.home or "inapplicable""#);
        assert_eq!(result, "inapplicable");
    }
}
