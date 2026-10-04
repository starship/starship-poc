//! The message protocol between the daemon and plugins.
//!
//! Each plugin is its own process. The daemon writes one [`Message`] holding
//! a [`Request`] per line to the plugin's stdin, and the plugin answers with
//! one [`Message`] holding a [`Response`] per line on stdout, using the
//! request's `id`. Both sides compile against these types, so the contract
//! can't drift between the SDK and the runtime.
//!
//! Plugin work happens inside a render: [`Request::BeginRender`] sends the
//! [`RenderContext`] once, [`Request::Call`]s refer to it by render ID, and
//! [`Request::EndRender`] lets the plugin drop it.
//!
//! Bump [`ABI_VERSION`] on any breaking change to these types. The daemon
//! refuses plugins built against a different version.

use std::collections::HashMap;
use std::path::PathBuf;

use serde::{Deserialize, Serialize};
use serde_json::Value;

/// Version of the plugin protocol. Plugins report it in their [`Manifest`].
pub const ABI_VERSION: u32 = 4;

/// The shell's state for one render. The client sends it to the daemon, and
/// the daemon sends it to each plugin once when the render begins. Plugins
/// read pwd and environment from here, never from their own process.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct RenderContext {
    /// The shell's working directory.
    pub pwd: PathBuf,
    /// The shell's environment variables.
    pub env: HashMap<String, String>,
}

/// One line on the wire: a request or response, and the ID that pairs them.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Message<T> {
    pub id: u64,
    pub body: T,
}

/// A message from the daemon to a plugin.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum Request {
    /// Ask for the plugin's [`Manifest`]. Sent once, at startup.
    Describe {
        /// Where the plugin may keep caches across daemon restarts, in a
        /// subdirectory named after itself. `None` keeps caches in memory.
        cache_dir: Option<PathBuf>,
    },
    /// Start a render. The plugin keeps `context` until [`Request::EndRender`]
    /// and answers with its applicability.
    BeginRender {
        render_id: u64,
        context: RenderContext,
    },
    /// Call a method listed in the plugin's [`Manifest`] within a render.
    Call { render_id: u64, method: String },
    /// Finish a render. A notification: the plugin sends no response.
    EndRender { render_id: u64 },
}

impl Request {
    /// Whether the daemon waits for a [`Response`] to this request.
    pub fn expects_response(&self) -> bool {
        !matches!(self, Self::EndRender { .. })
    }
}

/// A plugin's answer to a [`Request`], one variant per request that expects
/// one.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub enum Response {
    Describe(Manifest),
    BeginRender {
        /// Whether the plugin applies to this render. When `false`, the
        /// daemon sends no `Call`s for it.
        applicable: bool,
        /// A VCS plugin's distance from pwd to its nearest sentinel. `None`
        /// for general plugins and VCS plugins that don't detect here.
        depth: Option<u32>,
    },
    /// The method's return value. `Null` becomes `nil` in Lua.
    Call(Value),
}

/// Which contract a plugin implements.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum PluginKind {
    /// Implements `Plugin`.
    General,
    /// Implements `VcsPlugin` and reports a depth when a render begins.
    Vcs,
}

/// What a plugin is and what it exposes, reported once at startup.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Manifest {
    pub abi_version: u32,
    pub name: String,
    pub kind: PluginKind,
    /// Other plugins this one supersedes when both apply, e.g. jj over git.
    /// Empty for most plugins.
    pub shadows: Vec<String>,
    /// Methods callable through [`Request::Call`].
    pub methods: Vec<String>,
}
