use anyhow::{Context, Result};
use starship_common::{ShellContext, init_tracing, socket, styled::StyledContent};
use std::io::{BufRead, BufReader, Write};
use tracing::instrument;

#[instrument]
fn main() -> Result<()> {
    let _guard = init_tracing();
    run()
}

#[instrument(name = "starship")]
fn run() -> Result<()> {
    let mut stream = socket::connect()?;
    let shell_context = construct_shell_context();
    let request_json = serde_json::to_string(&shell_context)?;
    writeln!(stream, "{request_json}")?;
    stream.flush()?;

    let reader = BufReader::new(stream);
    let line = reader
        .lines()
        .next()
        .context("Failed to read line")?
        .context("No response from daemon")?;
    let prompt: StyledContent = serde_json::from_str(&line).context("Failed to parse response")?;
    print!("{}", render_prompt(prompt));

    Ok(())
}

fn construct_shell_context() -> ShellContext {
    let pwd = std::env::current_dir().ok();
    let user = std::env::var("USER").ok();

    ShellContext { pwd, user }
}

fn render_prompt(prompt: StyledContent) -> String {
    match prompt {
        StyledContent::Text(text) => text,
        StyledContent::Styled { children, .. } => {
            // TODO: Apply styles to the output
            let mut output = String::new();
            for child in children {
                output.push_str(&render_prompt(child));
            }
            output
        }
    }
}
