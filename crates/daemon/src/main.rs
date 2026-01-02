use std::{io::{BufRead, BufReader}, os::unix::net::UnixListener};

use anyhow::{Context, Result};
use starship_common::{ShellContext, socket_path};

fn main() -> Result<()> {
    tracing_subscriber::fmt()
        .with_max_level(tracing::Level::INFO)
        .pretty()
        .init();

      let socket_path = socket_path()?;
      
      // Remove old socket if it exists
    let _ = std::fs::remove_file(&socket_path);
    let listener = UnixListener::bind(&socket_path)
        .with_context(|| format!("failed to bind to socket: {:?}", &socket_path))?;
    tracing::info!("Listening on {:?}", &socket_path);

    for stream in listener.incoming() {
        match stream {
            Ok(stream) => {
                let mut reader = BufReader::new(stream);
                for line in reader.lines() {
                    let line = line?;
                    let request: ShellContext = serde_json::from_str(&line)?;
                    tracing::info!("Received request: {:?}", request);
                }

            }
            Err(e) => {
                tracing::error!("Error accepting connection: {}", e);
            }
        }
    }

    Ok(())
}
