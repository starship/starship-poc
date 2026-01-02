use std::{
    io::Write,
    os::unix::net::{UnixListener, UnixStream},
};

use anyhow::{Context, Result};
use serde_json;
use starship_common::{ShellContext, socket_path};

fn main() -> Result<()> {
    tracing_subscriber::fmt()
        .with_max_level(tracing::Level::INFO)
        .pretty()
        .init();

    let socket_path = socket_path()?;
    let mut stream = UnixStream::connect(&socket_path)
        .with_context(|| format!("failed to connect to daemon at {:?}", &socket_path))?;

    let pwd = std::env::current_dir().context("failed to get current directory")?;
    let user = std::env::var("USER").context("failed to get user")?;

    let request = ShellContext { pwd, user };
    let request_json = serde_json::to_string(&request)?;
    writeln!(stream, "{}", request_json)?;
    stream.flush()?;

    tracing::info!("Connected to daemon at {:?}", &socket_path);

    Ok(())
}
