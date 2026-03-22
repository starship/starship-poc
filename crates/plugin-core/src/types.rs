//! Shared types for communication between prompt<->daemon and daemon<->plugins.
//!
//! All types use serde for JSON serialization.

use serde::{Deserialize, Serialize};

/// Request from the prompt binary to the daemon.
/// Contains the shell context needed to render the prompt.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PromptRequest {
    pub pwd: String,
    pub user: String,
}

/// Response from the daemon to the prompt binary.
/// Contains the fully rendered prompt string (with ANSI colors).
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PromptResponse {
    pub prompt: String,
}

// These types cross the WASM boundary and use JSON for serialization.
//
// IMPORTANT: These support schema evolution - you can add new fields with
// #[serde(default)] and old plugins will continue to work (they'll just
// ignore the new fields). Similarly, new code can read old data (missing
// fields get their default values).
//
// Example of adding a field in the future:
//
//   pub struct PluginContext {
//       pub pwd: String,
//       pub user: String,
//       #[serde(default)]           // <-- This attribute is key!
//       pub exit_code: Option<i32>, // <-- New field, defaults to None
//   }

/// Context passed from daemon to each plugin's render function.
/// Plugins can use this directly, or call host functions for more data.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PluginContext {
    pub pwd: String,
    pub user: String,
}

/// Result returned by a plugin's render function.
/// Each plugin returns a "section" of the prompt.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PluginOutput {
    /// The rendered text (may include ANSI escape codes for colors)
    pub text: String,
}

/// Metadata about a plugin, returned by Plugin::metadata().
/// Used by the daemon for logging and plugin management.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PluginMetadata {
    pub name: String,
    pub description: String,
}
