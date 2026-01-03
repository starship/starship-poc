use std::{
    io::{BufRead, BufReader, Write},
    os::unix::net::{UnixListener, UnixStream},
};

use anyhow::{Context, Result};
use starship_common::{Module, Prompt, ShellContext, socket_path};

#[cfg(feature = "dhat-heap")]
#[global_allocator]
static ALLOC: dhat::Alloc = dhat::Alloc;

fn main() -> Result<()> {
    #[cfg(feature = "dhat-heap")]
    let _profiler = dhat::Profiler::new_heap();

    init_tracing();
    let socket_path = socket_path()?;

    // Remove old socket if it exists
    let _ = std::fs::remove_file(&socket_path);
    let listener = UnixListener::bind(&socket_path)
        .with_context(|| format!("failed to bind to socket: {:?}", &socket_path))?;
    tracing::info!("Listening on {:?}", &socket_path);

    #[cfg(feature = "dhat-heap")]
    let max_requests: usize = std::env::var("DHAT_REQUESTS")
        .ok()
        .and_then(|v| v.parse().ok())
        .unwrap_or(100);

    #[cfg(feature = "dhat-heap")]
    let mut request_count: usize = 0;

    for stream in listener.incoming() {
        match stream {
            Ok(mut stream) => {
                if let Err(e) = handle_client(&mut stream) {
                    tracing::error!("Error handling client: {}", e);
                }

                #[cfg(feature = "dhat-heap")]
                {
                    request_count += 1;
                    if request_count >= max_requests {
                        tracing::info!("dhat: reached {} requests, exiting", max_requests);
                        break;
                    }
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
    let pwd = context.pwd.unwrap_or_default().to_string_lossy().to_string();

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
