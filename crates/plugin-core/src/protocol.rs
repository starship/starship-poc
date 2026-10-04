//! The message protocol between the daemon (host) and plugins (guest).
//!
//! A plugin exports one function, `_plugin_handle`, which takes a JSON-encoded
//! [`Request`] and returns a JSON-encoded [`Response`]. The host exports one
//! function, `_host_handle`, which takes a [`HostRequest`] and returns a
//! [`HostResponse`]. Both sides compile against these types, so the contract
//! can't drift between the SDK and the runtime.
//!
//! Bump [`ABI_VERSION`] on any breaking change to these types. The host
//! refuses to load plugins built against a different version.

use std::collections::HashMap;
use std::path::PathBuf;

use serde::{Deserialize, Serialize};
use serde_json::Value;

/// Version of the plugin protocol. Plugins report it in their [`Manifest`].
pub const ABI_VERSION: u32 = 2;

/// The shell's state for one render, sent with every request that does
/// plugin work. Plugins read pwd and environment from here, never from their
/// own process.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct RenderContext {
    /// The shell's working directory.
    pub pwd: PathBuf,
    /// The shell's environment variables.
    pub env: HashMap<String, String>,
}

/// A message from the host to a plugin.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum Request {
    /// Ask for the plugin's [`Manifest`]. Sent once, at load.
    Describe,
    /// Ask whether the plugin applies to this render.
    IsApplicable { context: RenderContext },
    /// Ask a VCS plugin for its distance to the nearest sentinel. General
    /// plugins answer `None`.
    DetectDepth { context: RenderContext },
    /// Call a method listed in the plugin's [`Manifest`].
    Call {
        method: String,
        context: RenderContext,
    },
}

/// A plugin's answer to a [`Request`], one variant per request.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub enum Response {
    Describe(Manifest),
    IsApplicable(bool),
    DetectDepth(Option<u32>),
    /// The method's return value. `Null` becomes `nil` in Lua.
    Call(Value),
}

/// Which contract a plugin implements.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum PluginKind {
    /// Implements `Plugin`.
    General,
    /// Implements `VcsPlugin` and answers `DetectDepth`.
    Vcs,
}

/// What a plugin is and what it exposes, reported once at load.
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

/// A message from a plugin to the host.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum HostRequest {
    /// Run a command in `cwd`. With `cached`, the host may return a stored
    /// result keyed by the binary and arguments.
    Exec {
        cmd: String,
        args: Vec<String>,
        cwd: PathBuf,
        cached: bool,
    },
    /// Check whether an absolute path exists.
    FileExists { path: PathBuf },
}

/// The host's answer to a [`HostRequest`], one variant per request.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum HostResponse {
    Exec(Option<String>),
    FileExists(bool),
}
