use anyhow::{Context, Result};
use starship_common::{styled::StyledContent, ShellContext};
use std::io::{BufRead, BufReader, Read, Write};

#[tracing::instrument(skip_all)]
pub fn run<S: Read + Write>(mut stream: S, context: &ShellContext) -> Result<String> {
    // Send the context to the daemon
    let request_json =
        tracing::info_span!("serialize_request").in_scope(|| serde_json::to_string(context))?;

    tracing::info_span!("send_request").in_scope(|| {
        writeln!(stream, "{request_json}")?;
        stream.flush()
    })?;

    // Read the response from the daemon
    let line = tracing::info_span!("await_response").in_scope(|| {
        BufReader::new(stream)
            .lines()
            .next()
            .context("Failed to read line")?
            .context("No response from daemon")
    })?;

    // Parse the response from the daemon
    let prompt: StyledContent = tracing::info_span!("deserialize_response")
        .in_scope(|| serde_json::from_str(&line).context("Failed to parse response"))?;

    let rendered = tracing::info_span!("render").in_scope(|| prompt.to_string());
    Ok(rendered)
}
