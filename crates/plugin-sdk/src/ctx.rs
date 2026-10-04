//! What a plugin knows about the render it's answering for.

use std::path::{Path, PathBuf};

use starship_plugin_core::RenderContext;

use crate::host;

/// The shell's state for one request: its working directory and
/// environment, plus helpers that use them.
///
/// Plugins read pwd and environment only through `Ctx`. The plugin's own
/// process cwd and environment belong to the daemon, not the shell.
pub struct Ctx {
    context: RenderContext,
}

impl Ctx {
    pub fn new(context: RenderContext) -> Self {
        Self { context }
    }

    /// The shell's working directory.
    pub fn pwd(&self) -> &Path {
        &self.context.pwd
    }

    /// An environment variable from the shell.
    pub fn env(&self, name: &str) -> Option<&str> {
        self.context.env.get(name).map(String::as_str)
    }

    /// `relative` joined onto the shell's working directory.
    pub fn path(&self, relative: impl AsRef<Path>) -> PathBuf {
        self.context.pwd.join(relative)
    }

    /// Whether `relative` exists in the shell's working directory.
    pub fn file_exists(&self, relative: impl AsRef<Path>) -> bool {
        host::file_exists(&self.path(relative))
    }

    /// Runs a command in the shell's working directory, reusing a cached
    /// result while the binary and arguments are unchanged.
    ///
    /// Use for output that depends only on the binary, like `node --version`.
    pub fn exec(&self, cmd: &str, args: &[&str]) -> Option<String> {
        host::exec(cmd, args, self.pwd(), true)
    }

    /// Runs a command in the shell's working directory without caching.
    ///
    /// Use for output that depends on more than the binary, like
    /// `git branch --show-current`.
    pub fn exec_uncached(&self, cmd: &str, args: &[&str]) -> Option<String> {
        host::exec(cmd, args, self.pwd(), false)
    }
}
