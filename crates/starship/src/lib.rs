use anyhow::{Context, Result};
use starship_common::{ShellContext, styled::StyledContent};
use std::io::{BufRead, BufReader, Read, Write};

pub fn run<S: Read + Write>(mut stream: S, context: &ShellContext) -> Result<String> {
    // Send the context to the daemon
    let request_json = serde_json::to_string(context)?;
    writeln!(stream, "{request_json}")?;
    stream.flush()?;

    // Read the response from the daemon
    let line = BufReader::new(stream)
        .lines()
        .next()
        .context("Failed to read line")?
        .context("No response from daemon")?;

    // Parse the response from the daemon
    let prompt: StyledContent = serde_json::from_str(&line).context("Failed to parse response")?;
    let rendered_prompt = render_prompt(prompt);
    Ok(rendered_prompt)
}

/// Render the structured prompt representation into a string.
fn render_prompt(prompt: StyledContent) -> String {
    match prompt {
        StyledContent::Text(text) => text,
        StyledContent::Styled { children, .. } => {
            // TODO: Apply styles to the output
            children
                .iter()
                .map(|child| render_prompt(child.clone()))
                .collect()
        }
    }
}
