//! SDK for building Starship WASM plugins.

pub use serde_json;
pub use starship_plugin_core;
pub use starship_plugin_core::{alloc, dealloc, read_msg, write_msg};
pub use starship_plugin_macros::export_plugin;

pub mod host;

/// Required contract for all Starship plugins.
///
/// Provides the plugin's identity and activation logic. The `#[export_plugin]`
/// macro references this trait to generate WASM exports, so failing to
/// implement it is a compile error.
///
/// Plugin-specific methods go in a separate `#[export_plugin] impl` block.
pub trait Plugin: Default {
    /// Unique identifier for the plugin.
    const NAME: &str;

    /// Whether the plugin should activate in the current directory.
    fn is_active(&self) -> bool;
}
