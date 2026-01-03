use std::{
    io::{BufRead, BufReader, Write},
    os::unix::net::{UnixListener, UnixStream},
};

use anyhow::{Context, Result};
use starship_common::{socket_path, Module, Prompt, ShellContext};

fn main() -> Result<()> {
    init_tracing();

    let socket_path = socket_path()?;

    let _ = std::fs::remove_file(&socket_path);
    let listener = UnixListener::bind(&socket_path)
        .with_context(|| format!("failed to bind to socket: {:?}", &socket_path))?;
    tracing::info!("Listening on {:?}", &socket_path);

    for stream in listener.incoming() {
        match stream {
            Ok(mut stream) => {
                if let Err(e) = handle_client(&mut stream) {
                    tracing::error!("Error handling client: {}", e);
                }
            }
            Err(e) => {
                tracing::error!("Error accepting connection: {}", e);
            }
        }
    }

    Ok(())
}

fn handle_client(stream: &mut UnixStream) -> Result<()> {
    let reader = BufReader::with_capacity(256, stream.try_clone()?);

    for line in reader.lines() {
        let line = line.context("Failed to read line")?;
        if line.is_empty() {
            continue;
        }

        let context: ShellContext =
            serde_json::from_str(&line).context("Failed to parse request")?;

        let response = handle_request(context)?;
        serde_json::to_writer(&mut *stream, &response)?;
        stream.write_all(b"\n")?;
        stream.flush()?;
    }

    Ok(())
}

fn handle_request(context: ShellContext) -> Result<Prompt> {
    let user = context.user.unwrap_or_default();
    let pwd = context
        .pwd
        .unwrap_or_default()
        .to_string_lossy()
        .to_string();

    let prompt = Prompt {
        left: vec![
            Module {
                name: "user".into(),
                output: user.into(),
            },
            Module {
                name: "directory".into(),
                output: pwd.into(),
            },
        ],
        right: vec![],
    };
    Ok(prompt)
}

fn init_tracing() {
    tracing_subscriber::fmt()
        .with_max_level(tracing::Level::INFO)
        .pretty()
        .init();
}
