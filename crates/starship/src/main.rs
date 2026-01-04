use std::{
    env,
    io::{BufRead, BufReader, Write},
};

use anyhow::{Context, Result};
use starship_common::{Prompt, ShellContext, socket};
use tracing::instrument;

#[instrument]
fn main() -> Result<()> {
    let _guard = init_tracing();
    run()
}

#[instrument(name = "starship")]
fn run() -> Result<()> {
    // Send the shell context to the daemon
    let mut stream = socket::connect()?;
    let shell_context = construct_shell_context();
    let request_json = serde_json::to_string(&shell_context)?;
    writeln!(stream, "{request_json}")?;
    stream.flush()?;

    // Receive the response from the daemon
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

/// Initialize tracing.
///
/// If the `STARSHIP_PROFILE` environment variable is set, the tracing output
/// will be formatted for profiling.
///
/// Returns a guard that shouldn't be dropped until the program exits.
fn init_tracing() -> Option<impl Drop> {
    if env::var("STARSHIP_PROFILE").is_ok() {
        let guard = tracing_profile::init_tracing().expect("Failed to initialize profiler");
        return Some(guard);
    }

    tracing_subscriber::fmt()
        .with_max_level(tracing::Level::INFO)
        .pretty()
        .init();
    None
}
