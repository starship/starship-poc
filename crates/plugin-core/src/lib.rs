//! Shared types and serialization helpers used by both the daemon (host) and plugins (guest).
//!
//! This crate compiles to both native and `wasm32-unknown-unknown` targets.
//! Keep dependencies minimal — anything added here ends up in every plugin binary.

pub mod bitwise;
pub mod guest;
pub mod types;

pub use bitwise::{from_bitwise, into_bitwise};
pub use guest::{alloc, dealloc, read_msg, write_msg};
pub use types::{PluginContext, PluginMetadata, PluginOutput, PromptRequest, PromptResponse};
