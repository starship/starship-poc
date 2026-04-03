use std::env;

/// Initialize tracing.
///
/// If `STARSHIP_PROFILE` is set, output is formatted for profiling.
///
/// Returns a guard that must not be dropped until the program exits.
///
/// # Panics
///
/// Panics if `STARSHIP_PROFILE` is set but the profiler fails to initialize.
#[must_use]
pub fn init_tracing() -> Option<impl Drop> {
    if env::var("STARSHIP_PROFILE").is_ok() {
        if env::var("RUST_LOG").is_err() {
            // SAFETY: called before any threads are spawned.
            unsafe { env::set_var("RUST_LOG", "starship=debug,warn") };
        }
        let guard = tracing_profile::init_tracing().expect("Failed to initialize profiler");
        return Some(guard);
    }

    tracing_subscriber::fmt()
        .with_env_filter(
            tracing_subscriber::EnvFilter::try_from_default_env()
                .unwrap_or_else(|_| "starship=debug,warn".parse().unwrap()),
        )
        .pretty()
        .init();
    None
}
