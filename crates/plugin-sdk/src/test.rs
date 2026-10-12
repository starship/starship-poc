//! Helpers for testing a plugin in-process, through the same request handling
//! the daemon uses. Enable the `testing` feature in `[dev-dependencies]`.
//!
//! ```ignore
//! use starship_plugin_sdk::{assert_plugin, testing::TestCtx};
//!
//! #[test]
//! fn reads_the_version() {
//!     let ctx = TestCtx::new().file("package.json", "{}").command("node", "v20.1.0");
//!     assert!(testing::applicable(&NodejsPlugin, &ctx));
//!     assert_plugin!(NodejsPlugin, ctx, "version" => Some("20.1.0"));
//! }
//! ```

use std::collections::HashMap;
use std::fs;
use std::path::{Path, PathBuf};
use std::process::Command;

use serde_json::Value;
use starship_plugin_core::{RenderContext, Request, Response};

use crate::dispatch::{Handler, Renders};

/// A render in its own temporary directory, for testing plugins.
///
/// The pwd is never the test process's cwd, so a plugin that reads a
/// relative path instead of using `ctx.path` fails its tests. The environment
/// starts with only the test process's `PATH`.
pub struct TestCtx {
    dir: tempfile::TempDir,
    env: HashMap<String, String>,
}

impl Default for TestCtx {
    fn default() -> Self {
        Self::new()
    }
}

impl TestCtx {
    pub fn new() -> Self {
        let dir = tempfile::tempdir().expect("create a temporary pwd");
        let env = std::env::var("PATH")
            .map(|path| HashMap::from([("PATH".to_string(), path)]))
            .unwrap_or_default();
        Self { dir, env }
    }

    /// The render's working directory.
    pub fn pwd(&self) -> &Path {
        self.dir.path()
    }

    /// Sets an environment variable for the render.
    #[must_use]
    pub fn env(mut self, name: &str, value: &str) -> Self {
        self.env.insert(name.to_string(), value.to_string());
        self
    }

    /// Writes a file relative to the pwd, creating parent directories.
    #[must_use]
    pub fn file(self, relative: &str, contents: &str) -> Self {
        let path = self.pwd().join(relative);
        if let Some(parent) = path.parent() {
            fs::create_dir_all(parent).expect("create parent directories");
        }
        fs::write(&path, contents).expect("write test file");
        self
    }

    /// Puts a fake `name` command first on the render's `PATH`. It prints
    /// `stdout` and succeeds.
    #[cfg(unix)]
    #[must_use]
    pub fn command(mut self, name: &str, stdout: &str) -> Self {
        use std::os::unix::fs::PermissionsExt;

        let bin = self.pwd().join(".test-bin");
        fs::create_dir_all(&bin).expect("create fake command dir");
        let script = bin.join(name);
        let quoted = stdout.replace('\'', r"'\''");
        fs::write(&script, format!("#!/bin/sh\nprintf '%s\\n' '{quoted}'\n"))
            .expect("write fake command");
        fs::set_permissions(&script, fs::Permissions::from_mode(0o755))
            .expect("make fake command executable");

        let path = match self.env.get("PATH") {
            Some(rest) => format!("{}:{rest}", bin.display()),
            None => bin.display().to_string(),
        };
        self.env.insert("PATH".to_string(), path);
        self
    }

    /// Makes the pwd a git repository on branch `main`.
    #[must_use]
    pub fn git_repo(self) -> Self {
        let status = Command::new("git")
            .args(["init", "--quiet", "--initial-branch=main"])
            .current_dir(self.pwd())
            .status()
            .expect("run git init");
        assert!(status.success(), "git init failed");
        self
    }

    /// The render context a plugin receives.
    pub fn context(&self) -> RenderContext {
        RenderContext {
            pwd: self.pwd().to_path_buf(),
            env: self.env.clone(),
        }
    }

    /// `relative` joined onto the pwd.
    pub fn path(&self, relative: &str) -> PathBuf {
        self.pwd().join(relative)
    }
}

/// Whether `plugin` applies to the render in `ctx`.
pub fn applicable(plugin: &impl Handler, ctx: &TestCtx) -> bool {
    begin(plugin, ctx).0
}

/// A VCS plugin's depth for the render in `ctx`.
pub fn depth(plugin: &impl Handler, ctx: &TestCtx) -> Option<u32> {
    begin(plugin, ctx).1
}

/// Reads `method` from `plugin` in one render, the way a config would. The
/// result is what Lua sees, so `nil` is `Value::Null`.
pub fn read(plugin: &impl Handler, ctx: &TestCtx, method: &str) -> Value {
    let renders = Renders::default();
    let begin = Request::BeginRender {
        render_id: 1,
        context: ctx.context(),
    };
    plugin.handle(&renders, begin);
    let call = Request::Call {
        render_id: 1,
        method: method.to_string(),
    };
    match plugin.handle(&renders, call) {
        Some(Response::Call(value)) => value,
        other => panic!("expected a Call response, got {other:?}"),
    }
}

fn begin(plugin: &impl Handler, ctx: &TestCtx) -> (bool, Option<u32>) {
    let request = Request::BeginRender {
        render_id: 1,
        context: ctx.context(),
    };
    match plugin.handle(&Renders::default(), request) {
        Some(Response::BeginRender { applicable, depth }) => (applicable, depth),
        other => panic!("expected a BeginRender response, got {other:?}"),
    }
}

/// Asserts what a plugin's methods return for one render, as Lua would see
/// them. Expected values go through serde, so `Some("x")` matches `"x"` and
/// `None::<&str>` matches `nil`.
///
/// ```ignore
/// assert_plugin!(NodejsPlugin, ctx, "version" => Some("20.1.0"));
/// ```
#[macro_export]
macro_rules! assert_plugin {
    ($plugin:expr, $ctx:expr, $($method:literal => $expected:expr),+ $(,)?) => {
        $(
            ::std::assert_eq!(
                $crate::testing::read(&$plugin, &$ctx, $method),
                $crate::dispatch::to_value($expected),
                "plugin method `{}`",
                $method,
            );
        )+
    };
}

#[cfg(test)]
mod tests {
    use serde_json::Value;

    use super::*;
    use crate::dispatch::{Methods, handle_plugin};
    use crate::{Ctx, Plugin};

    /// Checks for `Cargo.toml` relative to the plugin process's cwd: the bug
    /// `TestCtx` is meant to catch.
    #[derive(Default)]
    struct Careless;

    /// Checks for `Cargo.toml` in the render's pwd.
    #[derive(Default)]
    struct Careful;

    impl Plugin for Careless {
        const NAME: &str = "careless";

        fn is_applicable(&self, _ctx: &Ctx) -> bool {
            Path::new("Cargo.toml").exists()
        }
    }

    impl Plugin for Careful {
        const NAME: &str = "careful";

        fn is_applicable(&self, ctx: &Ctx) -> bool {
            ctx.file_exists("Cargo.toml")
        }
    }

    // What `#[export_plugin]` generates, written out because a crate can only
    // hold one exported plugin.
    macro_rules! no_methods {
        ($plugin:ty) => {
            impl Methods for $plugin {
                const METHODS: &'static [&'static str] = &[];

                fn call(&self, _method: &str, _ctx: &Ctx) -> Value {
                    Value::Null
                }
            }

            impl Handler for $plugin {
                fn handle(&self, renders: &Renders, request: Request) -> Option<Response> {
                    handle_plugin(self, renders, request)
                }
            }
        };
    }
    no_methods!(Careless);
    no_methods!(Careful);

    #[test]
    fn pwd_is_never_the_test_process_cwd() {
        let ctx = TestCtx::new();
        assert_ne!(ctx.pwd(), std::env::current_dir().unwrap());
    }

    #[test]
    fn relative_paths_fail_under_test_ctx() {
        // The test process's cwd is this crate, which has a Cargo.toml, so a
        // relative check passes by accident outside `TestCtx`.
        assert!(Path::new("Cargo.toml").exists());

        let empty = TestCtx::new();
        assert!(
            applicable(&Careless, &empty),
            "the careless plugin is fooled"
        );
        assert!(!applicable(&Careful, &empty));

        let project = TestCtx::new().file("Cargo.toml", "");
        assert!(applicable(&Careful, &project));
    }

    #[cfg(unix)]
    #[test]
    fn fake_commands_run_through_ctx() {
        let ctx = TestCtx::new().command("node", "v20.1.0");
        let output = Ctx::new(ctx.context()).exec_uncached("node", &["--version"]);
        assert_eq!(output.as_deref(), Some("v20.1.0\n"));
    }

    #[test]
    fn git_repo_creates_a_repository() {
        let ctx = TestCtx::new().git_repo();
        assert!(ctx.path(".git").is_dir());
    }
}
