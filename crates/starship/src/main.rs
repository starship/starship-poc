use anyhow::{Context, Result};
use starship_common::{init_tracing, socket, Prompt, ShellContext};
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
    let prompt: Prompt = serde_json::from_str(&line).context("Failed to parse response")?;
    print!("{} ❯", prompt.render());

    Ok(())
}

fn construct_shell_context() -> ShellContext {
    let pwd = std::env::current_dir().ok();
    let user = std::env::var("USER").ok();

    ShellContext { pwd, user }
}
