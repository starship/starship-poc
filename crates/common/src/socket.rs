use anyhow::{Context, Result};
use directories::UserDirs;
use tracing::instrument;
use std::{
    fs, os::unix::net::{UnixListener, UnixStream}, path::PathBuf
};

fn get_socket_path() -> Result<PathBuf> {
    let user_dirs = UserDirs::new().with_context(|| "Failed to get user directories")?;
    let config_dir = user_dirs.home_dir().join(".config");
    let socket_path = config_dir.join("starship/starship.sock");

    Ok(socket_path)
}

/// Listen to incoming connections from the prompt.
pub fn listen() -> Result<UnixListener> {
    let socket_path = get_socket_path()?;
    let _ = fs::remove_file(&socket_path);

    let listener = UnixListener::bind(&socket_path)
        .with_context(|| format!("failed to bind to socket: {}", &socket_path.display()))?;

    tracing::info!("Listening on {}", &socket_path.display());
    Ok(listener)
}

/// Connect to the daemon.
#[instrument]
pub fn connect() -> Result<UnixStream> {
    let socket_path = get_socket_path()?;
    let stream = UnixStream::connect(&socket_path)
        .with_context(|| format!("failed to connect to socket: {}", &socket_path.display()))?;

    tracing::info!("Connected to socket: {}", &socket_path.display());
    Ok(stream)
}
