use std::{
    io::{BufRead, BufReader, Write},
    os::unix::net::{UnixListener, UnixStream},
};

use anyhow::{Context, Result};
use starship_common::{socket_path, Module, Prompt, ShellContext};

fn main() -> Result<()> {
    init_tracing();
    let socket_path = socket_path()?;

    // Remove old socket if it exists
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
    let reader = BufReader::new(stream.try_clone()?);

    for line in reader.lines() {
        let line = line.context("Failed to read line")?;
        if line.is_empty() {
            continue;
        }

        let context: ShellContext =
            serde_json::from_str(&line).context("Failed to parse request")?;

        let response = handle_request(context)?;
        let response_json = serde_json::to_string(&response)?;
        writeln!(stream, "{}", response_json)?;
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
                name: "user".to_string(),
                output: user,
            },
            Module {
                name: "directory".to_string(),
                output: pwd,
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
