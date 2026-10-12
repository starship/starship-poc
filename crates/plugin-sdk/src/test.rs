//! Fixtures for testing a plugin in-process, through the same request
//! handling the daemon uses. Enable the `testing` feature in
//! `[dev-dependencies]`.
//!
//! A test sets up a [`Dir`], an [`Env`] and the [`Commands`] the plugin may
//! run, then renders the plugin with them:
//!
//! ```ignore
//! use starship_plugin_sdk::test::prelude::*;
//!
//! #[test]
//! fn reads_the_version_in_a_node_project() {
//!     let dir = Dir::new().file("package.json", "{}");
//!     let commands = Commands::new().ok("node --version", "v20.1.0\n");
//!
//!     let render = render!(NodejsPlugin { dir, commands });
//!
//!     assert!(render.applicable());
//!     assert_eq!(render.read("version"), "20.1.0");
//! }
//! ```

use std::cell::RefCell;
use std::collections::HashMap;
use std::fs;
use std::path::{Path, PathBuf};

use serde_json::Value;
use starship_plugin_core::{RenderContext, Request, Response};

use crate::dispatch::{Handler, Renders};

/// A temporary directory to render in. It's never the test process's cwd,
/// so a plugin that reads a relative path instead of using `ctx.path` fails
/// its tests.
pub struct Dir {
    root: tempfile::TempDir,
}

impl Default for Dir {
    fn default() -> Self {
        Self::new()
    }
}

impl Dir {
    pub fn new() -> Self {
        Self {
            root: tempfile::tempdir().expect("create a temporary directory"),
        }
    }

    /// The directory's absolute path.
    pub fn root(&self) -> &Path {
        self.root.path()
    }

    /// `relative` joined onto the directory.
    pub fn path(&self, relative: &str) -> PathBuf {
        self.root().join(relative)
    }

    /// Writes a file, creating parent directories.
    #[must_use]
    pub fn file(self, relative: &str, contents: &str) -> Self {
        let path = self.path(relative);
        if let Some(parent) = path.parent() {
            fs::create_dir_all(parent).expect("create parent directories");
        }
        fs::write(&path, contents).expect("write test file");
        self
    }

    /// Creates a directory and its parents.
    #[must_use]
    pub fn dir(self, relative: &str) -> Self {
        fs::create_dir_all(self.path(relative)).expect("create test directory");
        self
    }

    /// Makes the directory a git repository on branch `main`.
    #[must_use]
    pub fn git_repo(self) -> Self {
        let status = std::process::Command::new("git")
            .args(["init", "--quiet", "--initial-branch=main"])
            .current_dir(self.root())
            .status()
            .expect("run git init");
        assert!(status.success(), "git init failed");
        self
    }

    /// Writes an executable script that prints `stdout`, for end-to-end
    /// tests where a real plugin binary resolves commands on `PATH`.
    /// In-process tests use [`Commands`] instead.
    #[cfg(unix)]
    #[must_use]
    pub fn executable(self, relative: &str, stdout: &str) -> Self {
        use std::os::unix::fs::PermissionsExt;

        let quoted = stdout.replace('\'', r"'\''");
        let this = self.file(relative, &format!("#!/bin/sh\nprintf '%s' '{quoted}'\n"));
        fs::set_permissions(this.path(relative), fs::Permissions::from_mode(0o755))
            .expect("make script executable");
        this
    }
}

/// The shell's environment for a render. Starts empty.
#[derive(Debug, Clone, Default)]
pub struct Env {
    vars: HashMap<String, String>,
}

impl Env {
    pub fn new() -> Self {
        Self::default()
    }

    /// Sets a variable.
    #[must_use]
    pub fn var(mut self, name: &str, value: &str) -> Self {
        self.vars.insert(name.to_string(), value.to_string());
        self
    }
}

/// The commands a plugin may run during a render, with their mocked results.
///
/// While a [`Render`] drives a plugin, `ctx.exec` and `ctx.exec_uncached`
/// answer from these, matched on the full command line (`"node --version"`).
/// Nothing runs, and a command that isn't listed fails the test.
#[derive(Debug, Clone, Default)]
pub struct Commands {
    mocks: Vec<Mock>,
}

impl Commands {
    pub fn new() -> Self {
        Self::default()
    }

    /// `line` succeeds and prints `stdout`.
    #[must_use]
    pub fn ok(mut self, line: &str, stdout: &str) -> Self {
        self.mocks.push(Mock {
            line: line.to_string(),
            stdout: Some(stdout.to_string()),
        });
        self
    }

    /// `line` fails, as when the command isn't installed.
    #[must_use]
    pub fn fails(mut self, line: &str) -> Self {
        self.mocks.push(Mock {
            line: line.to_string(),
            stdout: None,
        });
        self
    }
}

/// One mocked command line.
#[derive(Debug, Clone)]
pub(crate) struct Mock {
    line: String,
    /// `None` when the command fails.
    stdout: Option<String>,
}

impl Mock {
    /// What `ctx.exec` returns for this command.
    pub(crate) fn output(self) -> Option<String> {
        self.stdout
    }
}

/// One render of plugin `P`. It owns the plugin and the fixtures it's given;
/// anything not given is an empty temporary directory, its root as pwd, an
/// empty environment, and no commands. Usually built with [`render!`].
///
/// [`render!`]: crate::render
pub struct Render<P> {
    plugin: P,
    dir: Dir,
    /// Relative to `dir`. `""` is its root.
    pwd: &'static str,
    env: Env,
    commands: Commands,
}

impl<P: Handler> Render<P> {
    pub fn new(plugin: P) -> Self {
        Self {
            plugin,
            dir: Dir::new(),
            pwd: "",
            env: Env::new(),
            commands: Commands::new(),
        }
    }

    /// Renders in `dir`.
    #[must_use]
    pub fn dir(mut self, dir: Dir) -> Self {
        self.dir = dir;
        self
    }

    /// Renders from a subdirectory of the render's [`Dir`].
    #[must_use]
    pub fn pwd(mut self, relative: &'static str) -> Self {
        self.pwd = relative;
        self
    }

    /// Uses `env` as the shell's environment.
    #[must_use]
    pub fn env(mut self, env: Env) -> Self {
        self.env = env;
        self
    }

    /// Lets the plugin run `commands`, and nothing else.
    #[must_use]
    pub fn commands(mut self, commands: Commands) -> Self {
        self.commands = commands;
        self
    }

    /// `relative` joined onto the render's [`Dir`].
    pub fn path(&self, relative: &str) -> PathBuf {
        self.dir.path(relative)
    }

    /// The render context the plugin receives.
    pub fn context(&self) -> RenderContext {
        RenderContext {
            pwd: self.dir.path(self.pwd),
            env: self.env.vars.clone(),
        }
    }

    /// Whether the plugin applies to this render.
    pub fn applicable(&self) -> bool {
        self.begin().0
    }

    /// A VCS plugin's depth for this render.
    pub fn depth(&self) -> Option<u32> {
        self.begin().1
    }

    /// Reads `method` from the plugin in this render, the way a config
    /// would. The result is what Lua sees: compare it with `assert_eq!`
    /// against a string, number or bool, and `nil` is [`Value::is_null`].
    pub fn read(&self, method: &str) -> Value {
        self.with_commands(|| {
            let renders = Renders::default();
            self.plugin.handle(&renders, self.begin_request());
            let call = Request::Call {
                render_id: 1,
                method: method.to_string(),
            };
            match self.plugin.handle(&renders, call) {
                Some(Response::Call(value)) => value,
                other => panic!("expected a Call response, got {other:?}"),
            }
        })
    }

    fn begin(&self) -> (bool, Option<u32>) {
        self.with_commands(|| {
            match self
                .plugin
                .handle(&Renders::default(), self.begin_request())
            {
                Some(Response::BeginRender { applicable, depth }) => (applicable, depth),
                other => panic!("expected a BeginRender response, got {other:?}"),
            }
        })
    }

    fn begin_request(&self) -> Request {
        Request::BeginRender {
            render_id: 1,
            context: self.context(),
        }
    }

    /// Runs `f` with this render's commands answering `ctx.exec`.
    fn with_commands<T>(&self, f: impl FnOnce() -> T) -> T {
        struct Restore(Option<Commands>);
        impl Drop for Restore {
            fn drop(&mut self) {
                ACTIVE_COMMANDS.with_borrow_mut(|active| *active = self.0.take());
            }
        }

        let previous =
            ACTIVE_COMMANDS.with_borrow_mut(|active| active.replace(self.commands.clone()));
        let _restore = Restore(previous);
        f()
    }
}

/// The render context for `dir` and `env`, for end-to-end tests that send it
/// to a real plugin binary.
pub fn context(dir: &Dir, env: &Env) -> RenderContext {
    RenderContext {
        pwd: dir.root().to_path_buf(),
        env: env.vars.clone(),
    }
}

/// Renders a plugin with the fixtures named inside the braces. Each field
/// sets that part of the [`Render`]: `dir`, `pwd`, `env`, `commands`. A field
/// on its own uses the local variable of the same name.
///
/// ```ignore
/// let render = render!(NodejsPlugin { dir, commands });
/// let render = render!(GitPlugin { dir, pwd: "src/deep" });
/// let render = render!(NodejsPlugin);
/// ```
#[macro_export]
macro_rules! render {
    ($plugin:path) => {
        $crate::test::Render::new(<$plugin as ::core::default::Default>::default())
    };
    ($plugin:path { $($field:ident $(: $value:expr)?),* $(,)? }) => {
        $crate::test::Render::new(<$plugin as ::core::default::Default>::default())
            $(.$field($crate::render!(@value $field $(: $value)?)))*
    };
    (@value $field:ident) => { $field };
    (@value $field:ident : $value:expr) => { $value };
}

/// Everything a plugin's tests need, in one import.
pub mod prelude {
    pub use super::{Commands, Dir, Env, Render};
    pub use crate::render;
}

thread_local! {
    /// The commands of the [`Render`] driving a plugin on this thread, if any.
    static ACTIVE_COMMANDS: RefCell<Option<Commands>> = const { RefCell::new(None) };
}

/// The mock matching `cmd args...`, while a [`Render`] drives a plugin on
/// this thread. `None` outside a render, where commands run for real.
///
/// # Panics
///
/// Panics when a render is active and its commands don't list this one.
pub(crate) fn mocked(cmd: &str, args: &[&str]) -> Option<Mock> {
    ACTIVE_COMMANDS.with_borrow(|active| {
        let commands = active.as_ref()?;
        let line = std::iter::once(cmd)
            .chain(args.iter().copied())
            .collect::<Vec<_>>()
            .join(" ");
        let Some(mock) = commands.mocks.iter().find(|mock| mock.line == line) else {
            panic!(
                "unmocked command `{line}`; add `.ok(\"{line}\", ...)` or `.fails(\"{line}\")` to the render's commands"
            );
        };
        Some(mock.clone())
    })
}

#[cfg(test)]
mod tests {
    use serde_json::Value;

    use super::*;
    use crate::dispatch::{Methods, handle_plugin, to_value};
    use crate::{Ctx, Plugin};

    /// Checks for `Cargo.toml` relative to the plugin process's cwd: the bug
    /// [`Dir`] is meant to catch.
    #[derive(Default)]
    struct Careless;

    /// Checks for `Cargo.toml` in the render's pwd, and reports `node`'s
    /// version.
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

    impl Methods for Careless {
        const METHODS: &'static [&'static str] = &[];

        fn call(&self, _method: &str, _ctx: &Ctx) -> Value {
            Value::Null
        }
    }

    impl Methods for Careful {
        const METHODS: &'static [&'static str] = &["node", "pwd"];

        fn call(&self, method: &str, ctx: &Ctx) -> Value {
            match method {
                "node" => to_value(ctx.exec("node", &["--version"])),
                "pwd" => to_value(ctx.pwd()),
                _ => Value::Null,
            }
        }
    }

    // What `#[export_plugin]` generates, written out because a crate can only
    // hold one exported plugin.
    impl Handler for Careless {
        fn handle(&self, renders: &Renders, request: Request) -> Option<Response> {
            handle_plugin(self, renders, request)
        }
    }

    impl Handler for Careful {
        fn handle(&self, renders: &Renders, request: Request) -> Option<Response> {
            handle_plugin(self, renders, request)
        }
    }

    #[test]
    fn dir_is_never_the_test_process_cwd() {
        let render = render!(Careful);

        assert_ne!(render.path(""), std::env::current_dir().unwrap());
    }

    #[test]
    fn relative_paths_fail_under_a_test_render() {
        // The test process's cwd is this crate, which has a Cargo.toml, so a
        // relative check passes by accident outside a test render.
        let dir = Dir::new().file("Cargo.toml", "");

        let careless = render!(Careless);
        let careful = render!(Careful);
        let careful_in_project = render!(Careful { dir });

        assert!(careless.applicable(), "the careless plugin is fooled");
        assert!(!careful.applicable());
        assert!(careful_in_project.applicable());
    }

    #[test]
    fn renders_can_start_in_a_subdirectory() {
        let dir = Dir::new().dir("src/deep");

        let render = render!(Careful {
            dir,
            pwd: "src/deep"
        });

        assert_eq!(render.read("pwd"), to_value(render.path("src/deep")));
    }

    #[test]
    fn commands_answer_from_the_render() {
        let node = Commands::new().ok("node --version", "v20.1.0\n");
        let no_node = Commands::new().fails("node --version");

        let with_node = render!(Careful { commands: node });
        let without_node = render!(Careful { commands: no_node });

        assert_eq!(with_node.read("node"), "v20.1.0\n");
        assert!(without_node.read("node").is_null());
    }

    #[test]
    #[should_panic(expected = "unmocked command `node --version`")]
    fn unlisted_commands_fail_the_test() {
        let render = render!(Careful);

        render.read("node");
    }

    #[test]
    fn env_is_empty_unless_given() {
        let env = Env::new().var("HOME", "/home/me");

        let bare = render!(Careful);
        let with_home = render!(Careful { env });

        assert!(bare.context().env.is_empty());
        assert_eq!(with_home.context().env["HOME"], "/home/me");
    }

    #[cfg(unix)]
    #[test]
    fn executables_print_their_output() {
        let dir = Dir::new().executable("bin/node", "v20.1.0\n");

        let output = std::process::Command::new(dir.path("bin/node"))
            .output()
            .unwrap();

        assert_eq!(output.stdout, b"v20.1.0\n");
    }

    #[test]
    fn git_repo_creates_a_repository() {
        let dir = Dir::new().git_repo();

        assert!(dir.path(".git").is_dir());
    }
}
