pub mod config;

use crate::config::Config;
use crate::config::ConfigLoader;
use anyhow::{Context, Result};
use starship_common::{Module, Prompt, ShellContext};
use std::io::{BufRead, BufReader, Read, Write};
use tracing::instrument;

#[must_use]
#[instrument(skip_all)]
pub fn handle_request(config: Config) -> Prompt {
    let prompt = config.format.unwrap_or_default();
    tracing::info!("prompt: {prompt}");

    Prompt {
        left: vec![
            Module {
                name: "user".into(),
                output: "".into(),
            },
            Module {
                name: "directory".into(),
                output: "".into(),
            },
        ],
        right: vec![],
    }
}

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
        let config = loader.load(&context)?;

        let response = handle_request(config);
        let writer = reader.get_mut();
        serde_json::to_writer(&mut *writer, &response)?;
        writer.write_all(b"\n")?;
        writer.flush()?;

        line.clear();
    }

    Ok(())
}
