//! What a plugin knows about the render it's answering for.

use std::path::{Path, PathBuf};
use std::process::Command;

use starship_plugin_core::RenderContext;

use crate::exec_cache;

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
        self.path(relative).exists()
    }

    /// Runs a command as the shell would, reusing a cached result while the
    /// binary and arguments are unchanged.
    ///
    /// Use for output that depends only on the binary, like `node --version`.
    pub fn exec(&self, cmd: &str, args: &[&str]) -> Option<String> {
        self.run(cmd, args, true)
    }

    /// Runs a command as the shell would, without caching.
    ///
    /// Use for output that depends on more than the binary, like
    /// `git branch --show-current`.
    pub fn exec_uncached(&self, cmd: &str, args: &[&str]) -> Option<String> {
        self.run(cmd, args, false)
    }

    /// Resolves `cmd` against the shell's `PATH` and runs it in the shell's
    /// working directory with the shell's environment. Returns stdout when
    /// the command succeeds.
    fn run(&self, cmd: &str, args: &[&str], cached: bool) -> Option<String> {
        let binary = which::which_in(cmd, self.env("PATH"), self.pwd()).ok()?;
        let cache = exec_cache::global();
        if cached && let Some(output) = cache.get(&binary, args) {
            return Some(output);
        }

        let output = Command::new(&binary)
            .args(args)
            .current_dir(self.pwd())
            .env_clear()
            .envs(&self.context.env)
            .output()
            .ok()
            .filter(|output| output.status.success())?;
        let stdout = String::from_utf8_lossy(&output.stdout).into_owned();

        if cached {
            cache.insert(&binary, args, stdout.clone());
        }
        Some(stdout)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn ctx_in(dir: &Path, env: &[(&str, &str)]) -> Ctx {
        Ctx::new(RenderContext {
            pwd: dir.to_path_buf(),
            env: env
                .iter()
                .map(|(name, value)| ((*name).to_string(), (*value).to_string()))
                .collect(),
        })
    }

    fn shell_path() -> String {
        std::env::var("PATH").unwrap()
    }

    #[test]
    fn exec_runs_with_the_shell_pwd_and_env() {
        let dir = tempfile::tempdir().unwrap();
        let path = shell_path();
        let ctx = ctx_in(dir.path(), &[("PATH", &path), ("GREETING", "hi")]);

        let output = ctx
            .exec_uncached("sh", &["-c", "echo \"$GREETING\"; pwd"])
            .unwrap();
        let mut lines = output.lines();
        assert_eq!(lines.next(), Some("hi"));
        assert_eq!(
            fs_canonical(Path::new(lines.next().unwrap())),
            fs_canonical(dir.path())
        );
    }

    #[test]
    fn exec_doesnt_leak_the_plugin_process_env() {
        let dir = tempfile::tempdir().unwrap();
        let path = shell_path();
        let ctx = ctx_in(dir.path(), &[("PATH", &path)]);
        // `CARGO` is set for the test process but not in this render context.
        let output = ctx.exec_uncached("sh", &["-c", "echo \"${CARGO:-unset}\""]);
        assert_eq!(output.as_deref(), Some("unset\n"));
    }

    #[test]
    fn exec_finds_nothing_without_a_shell_path() {
        let dir = tempfile::tempdir().unwrap();
        let ctx = ctx_in(dir.path(), &[]);
        assert_eq!(ctx.exec_uncached("sh", &["-c", "echo hi"]), None);
    }

    #[test]
    fn failing_command_returns_none() {
        let dir = tempfile::tempdir().unwrap();
        let path = shell_path();
        let ctx = ctx_in(dir.path(), &[("PATH", &path)]);
        assert_eq!(ctx.exec_uncached("sh", &["-c", "exit 1"]), None);
    }

    #[test]
    fn paths_resolve_against_the_shell_pwd() {
        let dir = tempfile::tempdir().unwrap();
        std::fs::write(dir.path().join("package.json"), "{}").unwrap();
        let ctx = ctx_in(dir.path(), &[]);
        assert!(ctx.file_exists("package.json"));
        assert!(!ctx.file_exists("Cargo.toml"));
        assert_eq!(ctx.path("package.json"), dir.path().join("package.json"));
    }

    fn fs_canonical(path: &Path) -> PathBuf {
        std::fs::canonicalize(path).unwrap()
    }
}
