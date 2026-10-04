//! The plugin protocol shared by the daemon and plugins.
//!
//! Keep dependencies minimal: anything added here ends up in every plugin
//! binary.

pub mod protocol;

pub use protocol::{ABI_VERSION, Manifest, Message, PluginKind, RenderContext, Request, Response};
