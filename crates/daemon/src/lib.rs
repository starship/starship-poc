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
            s.spawn(|| handle_client(server, &mut loader).unwrap());
            assert_eq!(
                starship::run(client, &ctx).unwrap(),
                "\x1b[32m/tmp $ \x1b[0m"
            );
        });
    }
}
