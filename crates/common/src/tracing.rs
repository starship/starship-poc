use std::env;

use tracing_subscriber::layer::SubscriberExt;
use tracing_subscriber::util::SubscriberInitExt;
use tracing_subscriber::{EnvFilter, Layer};

/// Initialize tracing.
///
/// If `STARSHIP_PROFILE` is set, output is formatted for profiling.
///
/// Returns a guard that must not be dropped until the program exits.
#[must_use]
pub fn init_tracing() -> Option<impl Drop> {
    let filter = || {
        EnvFilter::try_from_default_env().unwrap_or_else(|_| EnvFilter::new("starship=debug,warn"))
    };

    if env::var_os("STARSHIP_PROFILE").is_some() {
        // What `tracing_profile::init_tracing` installs, with our own filter
        // instead of one read from `RUST_LOG`.
        let (layer, guard) =
            tracing_profile::PrintTreeLayer::new(tracing_profile::PrintTreeConfig::default());
        tracing_subscriber::registry()
            .with(layer.with_filter(filter()))
            .init();
        return Some(guard);
    }

    tracing_subscriber::fmt()
        .with_env_filter(filter())
        .pretty()
        .init();
    None
}
