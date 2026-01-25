use anyhow::Result;
use starship_common::{init_tracing, socket};
use starship_daemon::{config::ConfigLoader, handle_client};

fn main() -> Result<()> {
    let _guard = init_tracing();

    // TODO: Setup as OnceCell
    let listener = socket::listen()?;
    let mut loader = ConfigLoader::new()?;

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
