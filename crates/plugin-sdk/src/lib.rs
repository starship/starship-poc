//! SDK for building Starship WASM plugins.

pub use serde_json;
pub use starship_plugin_core;
pub use starship_plugin_core::{alloc, dealloc, read_msg, write_msg};
pub use starship_plugin_macros::export_plugin;

pub mod host;
