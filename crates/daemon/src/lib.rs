pub mod config;

use crate::config::ConfigLoader;
use anyhow::{Context, Result};
use starship_common::ShellContext;
use std::io::{BufRead, BufReader, Read, Write};
use tracing::instrument;

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
        let output = loader.load(&context)?;

        let writer = reader.get_mut();
        serde_json::to_writer(&mut *writer, &output.format)?;
        writer.write_all(b"\n")?;
        writer.flush()?;

        line.clear();
    }

    Ok(())
}
