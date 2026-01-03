use std::{io::{BufRead, BufReader, Write}, os::unix::net::UnixStream};

use anyhow::{Context, Result};
use serde_json;
use starship_common::{Prompt, ShellContext, socket_path};

fn main() -> Result<()> {
    let socket_path = socket_path()?;
    let mut stream = UnixStream::connect(&socket_path)
        .with_context(|| format!("failed to connect to daemon at {:?}", &socket_path))?;

    // Send the shell context to the daemon
    let shell_context = construct_shell_context();
    let request_json = serde_json::to_string(&shell_context)?;
    writeln!(stream, "{}", request_json)?;
    stream.flush()?;

    // Receive the response from the daemon
    let reader = BufReader::new(stream);
    let line = reader.lines().next().context("Failed to read line")?.context("No response from daemon")?;
    let prompt: Prompt = serde_json::from_str(&line).context("Failed to parse response")?;
    print!("{} ❯", prompt.render());

    Ok(())
}

fn construct_shell_context() -> ShellContext {
    let pwd = std::env::current_dir().ok();
    let user = std::env::var("USER").ok();

    ShellContext { pwd, user }
}