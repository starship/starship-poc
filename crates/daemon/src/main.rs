use anyhow::Result;
use starship_common::socket;
use starship_daemon::{config::ConfigLoader, handle_client};

fn main() -> Result<()> {
    init_tracing();

    let mut loader = ConfigLoader::new()?;
    let listener = socket::listen()?;

    for stream in listener.incoming() {
        match stream {
            Ok(stream) => {
                if let Err(e) = handle_client(stream, &mut loader) {
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

fn init_tracing() {
    tracing_subscriber::fmt()
        .with_max_level(tracing::Level::INFO)
        .pretty()
        .init();
}
