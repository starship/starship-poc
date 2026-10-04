use std::cell::RefCell;
use std::collections::HashMap;
use std::io::{BufRead, BufReader, Write};
use std::path::{Path, PathBuf};
use std::process::{Child, ChildStderr, ChildStdin, ChildStdout, Command, Stdio};
use std::rc::Rc;

use anyhow::{Context, Result, bail, ensure};
use mlua::{Lua, LuaSerdeExt, Table};
use serde_json::Value;
use starship_plugin_core::{
    ABI_VERSION, Manifest, Message, PluginKind, RenderContext, Request, Response,
};
use tracing::instrument;

/// Plugin executables are named with this prefix, which keeps the daemon from
/// spawning unrelated binaries that share the plugin dir (e.g. `target/debug`).
const PLUGIN_PREFIX: &str = "starship-plugin-";

/// The daemon's ends of a plugin's stdin and stdout.
struct Pipes {
    stdin: ChildStdin,
    stdout: BufReader<ChildStdout>,
    next_id: u64,
}

impl Pipes {
    /// Sends one request and waits for its response.
    fn exchange(&mut self, request: &Request) -> Result<Response> {
        let id = self.next_id;
        self.next_id += 1;

        let mut line = serde_json::to_string(&Message { id, body: request })?;
        line.push('\n');
        self.stdin.write_all(line.as_bytes())?;
        self.stdin.flush()?;

        let mut reply = String::new();
        ensure!(
            self.stdout.read_line(&mut reply)? > 0,
            "plugin closed its stdout"
        );
        let response: Message<Response> = serde_json::from_str(&reply)?;
        ensure!(
            response.id == id,
            "plugin answered request {} while {id} was pending",
            response.id
        );
        Ok(response.body)
    }
}

/// A running plugin: one long-lived child process speaking the
/// [`Request`]/[`Response`] protocol over its stdin and stdout.
///
/// The plugin's [`Manifest`] is read once at startup, so its name, kind, and
/// methods are plain fields afterward.
pub struct PluginProcess {
    child: Child,
    /// `None` only while dropping, so stdin closes before the process is
    /// reaped.
    pipes: Option<Pipes>,
    manifest: Manifest,
}

impl PluginProcess {
    /// Starts the plugin executable at `path` and reads its manifest.
    /// `cache_dir` is where the plugin may persist caches.
    #[instrument(skip(cache_dir))]
    pub fn spawn(path: &Path, cache_dir: Option<&Path>) -> Result<Self> {
        let mut child = Command::new(path)
            .stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .stderr(Stdio::piped())
            .spawn()
            .with_context(|| format!("failed to start {}", path.display()))?;
        let label = path.file_name().unwrap_or_default().to_string_lossy();
        if let Some(stderr) = child.stderr.take() {
            forward_stderr(label.into_owned(), stderr);
        }
        let mut pipes = Pipes {
            stdin: child.stdin.take().context("plugin stdin")?,
            stdout: BufReader::new(child.stdout.take().context("plugin stdout")?),
            next_id: 0,
        };

        let request = Request::Describe {
            cache_dir: cache_dir.map(Path::to_path_buf),
        };
        let Response::Describe(manifest) = pipes.exchange(&request)? else {
            bail!("plugin answered Describe with an unexpected response");
        };
        ensure!(
            manifest.abi_version == ABI_VERSION,
            "plugin '{}' uses ABI version {}, expected {ABI_VERSION}",
            manifest.name,
            manifest.abi_version,
        );

        Ok(Self {
            child,
            pipes: Some(pipes),
            manifest,
        })
    }

    pub fn name(&self) -> &str {
        &self.manifest.name
    }

    pub fn kind(&self) -> PluginKind {
        self.manifest.kind
    }

    /// Other plugins this one supersedes when both apply.
    pub fn shadows(&self) -> &[String] {
        &self.manifest.shadows
    }

    /// Whether the plugin applies to the render described by `context`. A
    /// failing plugin is treated as inapplicable.
    #[instrument(skip_all, fields(plugin = %self.manifest.name))]
    pub fn is_applicable(&mut self, context: &RenderContext) -> bool {
        let request = Request::IsApplicable {
            context: context.clone(),
        };
        matches!(self.send(&request), Some(Response::IsApplicable(true)))
    }

    /// Distance from the render's pwd to the plugin's VCS sentinel. `None`
    /// for general plugins and VCS plugins that don't detect here.
    pub fn detect_depth(&mut self, context: &RenderContext) -> Option<u32> {
        let request = Request::DetectDepth {
            context: context.clone(),
        };
        match self.send(&request)? {
            Response::DetectDepth(depth) => depth,
            _ => None,
        }
    }

    /// Calls a method from the plugin's manifest. Returns `Null` for unknown
    /// methods and for failures, which are logged.
    #[instrument(skip(self, context), fields(plugin = %self.manifest.name))]
    pub fn call_method(&mut self, context: &RenderContext, method: &str) -> Value {
        if !self.manifest.methods.iter().any(|m| m == method) {
            return Value::Null;
        }
        let request = Request::Call {
            method: method.to_string(),
            context: context.clone(),
        };
        match self.send(&request) {
            Some(Response::Call(value)) => value,
            _ => Value::Null,
        }
    }

    /// Sends one request. A crashed plugin, a closed pipe, or an unparseable
    /// response is logged and becomes `None`; a mismatched response variant
    /// is treated as a failure by callers.
    fn send(&mut self, request: &Request) -> Option<Response> {
        self.pipes
            .as_mut()?
            .exchange(request)
            .inspect_err(|error| {
                tracing::error!(plugin = %self.manifest.name, ?request, %error, "plugin request failed");
            })
            .ok()
    }
}

impl Drop for PluginProcess {
    fn drop(&mut self) {
        // Closing stdin asks the plugin to exit; kill it in case it doesn't.
        drop(self.pipes.take());
        let _ = self.child.kill();
        let _ = self.child.wait();
    }
}

/// Logs each line a plugin writes to stderr under the plugin's name.
fn forward_stderr(plugin: String, stderr: ChildStderr) {
    let spawned = std::thread::Builder::new()
        .name(format!("{plugin}-stderr"))
        .spawn(move || {
            for line in BufReader::new(stderr).lines().map_while(Result::ok) {
                tracing::info!(%plugin, "{line}");
            }
        });
    if let Err(error) = spawned {
        tracing::warn!(%error, "failed to forward plugin stderr");
    }
}

/// The render in progress, shared by every plugin's Lua proxy.
#[derive(Default)]
pub struct RenderState {
    context: RenderContext,
    /// Each plugin's applicability, asked at most once per render.
    applicable: HashMap<String, bool>,
}

impl RenderState {
    /// Starts a new render, forgetting everything from the previous one.
    pub fn begin(&mut self, context: RenderContext) {
        self.context = context;
        self.applicable.clear();
    }
}

/// Registers a plugin as a Lua global with an `__index` metamethod.
///
/// Accessing `plugin_name.field` in Lua calls the plugin method of that name
/// for the render in `render`. Skips registration (with a warning) if the name
/// collides with an existing global.
pub fn register_plugin(
    lua: &Lua,
    plugin: Rc<RefCell<PluginProcess>>,
    render: Rc<RefCell<RenderState>>,
) -> mlua::Result<()> {
    let name = plugin.borrow().name().to_string();
    let lua_name = name.clone();

    if lua.globals().contains_key(name.as_str())? {
        tracing::warn!(plugin = %name, "plugin name collides with an existing Lua global, skipping");
        return Ok(());
    }

    let proxy: Table = lua.create_table()?;
    let meta: Table = lua.create_table()?;

    meta.set(
        "__index",
        lua.create_function(move |lua, (_table, key): (Table, String)| {
            let mut plugin = plugin.borrow_mut();
            let mut render = render.borrow_mut();
            let RenderState {
                context,
                applicable,
            } = &mut *render;
            let is_applicable = *applicable
                .entry(name.clone())
                .or_insert_with(|| plugin.is_applicable(context));
            if !is_applicable {
                return Ok(mlua::Value::Nil);
            }
            lua.to_value(&plugin.call_method(context, &key))
        })?,
    )?;

    proxy.set_metatable(Some(meta))?;
    lua.globals().set(lua_name, proxy)?;
    Ok(())
}

/// Starts every plugin executable in `plugin_dir`, in parallel.
///
/// Returns an empty vec if the directory doesn't exist. Logs and skips
/// plugins that fail to start or report a different ABI version.
#[instrument(skip(cache_dir))]
pub fn load_plugins(plugin_dir: &Path, cache_dir: Option<&Path>) -> Vec<PluginProcess> {
    let Ok(entries) = std::fs::read_dir(plugin_dir) else {
        return vec![];
    };
    let mut paths: Vec<PathBuf> = entries
        .filter_map(std::result::Result::ok)
        .map(|entry| entry.path())
        .filter(|path| is_plugin_executable(path))
        .collect();
    paths.sort();

    std::thread::scope(|scope| {
        let spawning: Vec<_> = paths
            .iter()
            .map(|path| (path, scope.spawn(|| PluginProcess::spawn(path, cache_dir))))
            .collect();
        spawning
            .into_iter()
            .filter_map(|(path, handle)| {
                let result = handle
                    .join()
                    .unwrap_or_else(|_| Err(anyhow::anyhow!("plugin startup panicked")));
                result
                    .inspect_err(|error| {
                        tracing::error!(path = %path.display(), %error, "failed to start plugin");
                    })
                    .ok()
            })
            .collect()
    })
}

/// Whether `path` is an executable file named like a plugin.
fn is_plugin_executable(path: &Path) -> bool {
    let named_like_plugin = path
        .file_name()
        .and_then(|name| name.to_str())
        .is_some_and(|name| name.starts_with(PLUGIN_PREFIX));
    named_like_plugin && is_executable(path)
}

#[cfg(unix)]
fn is_executable(path: &Path) -> bool {
    use std::os::unix::fs::PermissionsExt;
    path.metadata()
        .is_ok_and(|metadata| metadata.is_file() && metadata.permissions().mode() & 0o111 != 0)
}

#[cfg(not(unix))]
fn is_executable(path: &Path) -> bool {
    path.is_file() && path.extension().is_some_and(|ext| ext == "exe")
}

#[cfg(any(test, feature = "testing"))]
pub mod test_helpers {
    use std::path::{Path, PathBuf};

    use starship_plugin_core::RenderContext;

    use super::PluginProcess;

    /// The workspace's built plugin executable named `package`, e.g.
    /// `starship-plugin-test-harness`.
    pub fn plugin_binary(package: &str) -> PathBuf {
        Path::new(env!("PLUGIN_BIN_DIR")).join(format!("{package}{}", std::env::consts::EXE_SUFFIX))
    }

    /// A render context for `pwd` with the test process's environment.
    pub fn render_context(pwd: &Path) -> RenderContext {
        RenderContext {
            pwd: pwd.to_path_buf(),
            env: std::env::vars().collect(),
        }
    }

    /// A running plugin, rendering in its own temporary working directory.
    pub struct PluginFixture {
        pub dir: PathBuf,
        binary: PathBuf,
        plugin: PluginProcess,
        _tempdir: tempfile::TempDir,
    }

    impl PluginFixture {
        /// The general test plugin (`test`), which exercises every `Ctx` helper.
        pub fn test_harness() -> Self {
            Self::from_binary(plugin_binary("starship-plugin-test-harness"))
        }

        /// The stub VCS plugin (`vcs-test`).
        pub fn vcs_test_harness() -> Self {
            Self::from_binary(plugin_binary("starship-plugin-vcs-test-harness"))
        }

        pub fn from_binary(binary: PathBuf) -> Self {
            let dir = tempfile::TempDir::new().expect("tempdir");
            let plugin = PluginProcess::spawn(&binary, None).expect("plugin should start");
            Self {
                dir: dir.path().to_path_buf(),
                binary,
                plugin,
                _tempdir: dir,
            }
        }

        fn context(&self) -> RenderContext {
            render_context(&self.dir)
        }

        /// Calls a method for a render in [`dir`](Self::dir), returning
        /// strings as-is and other values as JSON.
        pub fn get(&mut self, method: &str) -> Option<String> {
            self.get_in(&self.context(), method)
        }

        /// Calls a method for the render described by `context`.
        pub fn get_in(&mut self, context: &RenderContext, method: &str) -> Option<String> {
            match self.plugin.call_method(context, method) {
                serde_json::Value::Null => None,
                serde_json::Value::String(s) => Some(s),
                other => Some(other.to_string()),
            }
        }

        pub fn is_applicable(&mut self) -> bool {
            let context = self.context();
            self.plugin.is_applicable(&context)
        }

        pub fn kind(&self) -> starship_plugin_core::PluginKind {
            self.plugin.kind()
        }

        pub fn shadows(&self) -> Vec<String> {
            self.plugin.shadows().to_vec()
        }

        pub fn detect_depth(&mut self) -> Option<u32> {
            let context = self.context();
            self.plugin.detect_depth(&context)
        }

        pub fn name(&self) -> &str {
            self.plugin.name()
        }

        /// Renders `return <lua_expr>` with this plugin registered.
        pub fn render(&mut self, lua_expr: &str) -> String {
            use crate::config::{Config, ConfigLoader};
            use starship_common::ShellContext;

            let lua_src = format!(r"return {lua_expr}");
            let plugin = PluginProcess::spawn(&self.binary, None).expect("plugin should start");
            let mut loader = ConfigLoader::from_source_with_plugins(&lua_src, vec![plugin])
                .expect("loader should build");
            let ctx = ShellContext {
                pwd: Some(self.dir.clone()),
                user: Some("test".into()),
                env: std::env::vars().collect(),
            };
            let func = loader.load(&ctx).expect("config should load");
            let output: Config = func.call(()).expect("lua should evaluate");
            output.format.to_string()
        }
    }
}

#[cfg(test)]
mod tests {
    use std::fs;
    use std::process::{Command, Stdio};
    use std::time::{Duration, Instant};

    use mlua::{Lua, LuaOptions, StdLib};

    use super::test_helpers::{PluginFixture, plugin_binary, render_context};
    use super::{PluginKind, PluginProcess, load_plugins};

    const TEST_HARNESS: &str = "starship-plugin-test-harness";

    #[test]
    fn sandboxed_luau_supports_index_metamethod() {
        let lua = Lua::new_with(StdLib::ALL_SAFE, LuaOptions::default()).unwrap();
        lua.sandbox(true).unwrap();
        let proxy = lua.create_table().unwrap();
        let meta = lua.create_table().unwrap();
        meta.set(
            "__index",
            lua.create_function(|_, (_table, key): (mlua::Table, String)| {
                Ok(format!("resolved:{key}"))
            })
            .unwrap(),
        )
        .unwrap();
        let _ = proxy.set_metatable(Some(meta));
        lua.globals().set("test_proxy", proxy).unwrap();
        let result: String = lua.load("return test_proxy.hello").eval().unwrap();
        assert_eq!(result, "resolved:hello");
    }

    #[test]
    fn plugin_loads_with_declared_name() {
        let plugin = PluginFixture::test_harness();
        assert_eq!(plugin.name(), "test");
    }

    #[test]
    fn unknown_method_returns_null() {
        let mut plugin = PluginFixture::test_harness();
        assert!(plugin.get("does_not_exist").is_none());
    }

    #[test]
    fn load_plugins_empty_dir_returns_empty_vec() {
        let plugin_dir = tempfile::tempdir().expect("plugin dir");
        assert!(load_plugins(plugin_dir.path(), None).is_empty());
    }

    #[cfg(unix)]
    #[test]
    fn load_plugins_only_starts_plugin_executables() {
        use std::os::unix::fs::PermissionsExt;

        let plugin_dir = tempfile::tempdir().unwrap();
        std::os::unix::fs::symlink(
            plugin_binary(TEST_HARNESS),
            plugin_dir.path().join(TEST_HARNESS),
        )
        .unwrap();
        // An executable that isn't a plugin, like `starship-daemon` sitting in
        // `target/debug`. It leaves a marker if anything runs it.
        let marker = plugin_dir.path().join("ran");
        let other = plugin_dir.path().join("starship-daemon");
        fs::write(&other, format!("#!/bin/sh\ntouch '{}'\n", marker.display())).unwrap();
        fs::set_permissions(&other, fs::Permissions::from_mode(0o755)).unwrap();

        let plugins = load_plugins(plugin_dir.path(), None);
        let names: Vec<_> = plugins.iter().map(PluginProcess::name).collect();
        assert_eq!(names, ["test"]);
        assert!(!marker.exists());
    }

    #[test]
    fn plugin_exits_when_stdin_closes() {
        let mut child = Command::new(plugin_binary(TEST_HARNESS))
            .stdin(Stdio::piped())
            .stdout(Stdio::null())
            .spawn()
            .unwrap();
        drop(child.stdin.take());

        let deadline = Instant::now() + Duration::from_secs(5);
        let status = loop {
            if let Some(status) = child.try_wait().unwrap() {
                break status;
            }
            assert!(Instant::now() < deadline, "plugin didn't exit on EOF");
            std::thread::sleep(Duration::from_millis(10));
        };
        assert!(status.success());
    }

    #[test]
    fn crashed_plugin_is_inapplicable_and_returns_nil() {
        let dir = tempfile::tempdir().unwrap();
        fs::write(dir.path().join(".starship-test-marker"), "").unwrap();
        let context = render_context(dir.path());
        let mut plugin = PluginProcess::spawn(&plugin_binary(TEST_HARNESS), None).unwrap();
        assert!(plugin.is_applicable(&context));

        plugin.child.kill().unwrap();
        plugin.child.wait().unwrap();
        assert!(!plugin.is_applicable(&context));
        assert_eq!(
            plugin.call_method(&context, "home"),
            serde_json::Value::Null
        );
    }

    #[test]
    fn plugin_reads_env_from_render_context() {
        let mut plugin = PluginFixture::test_harness();
        let mut context = render_context(&plugin.dir);
        context.env.insert("HOME".into(), "/home/from-shell".into());
        assert_eq!(
            plugin.get_in(&context, "home").as_deref(),
            Some("/home/from-shell")
        );

        context.env.remove("HOME");
        assert_eq!(plugin.get_in(&context, "home"), None);
    }

    #[test]
    fn requests_use_their_own_render_context() {
        let mut plugin = PluginFixture::test_harness();
        let other = tempfile::tempdir().unwrap();
        let pwd_in = |plugin: &mut PluginFixture, dir: &std::path::Path| {
            let output = plugin
                .get_in(&render_context(dir), "pwd")
                .expect("pwd output");
            fs::canonicalize(output).unwrap()
        };

        let dir = plugin.dir.clone();
        let first = pwd_in(&mut plugin, &dir);
        let second = pwd_in(&mut plugin, other.path());
        assert_eq!(first, fs::canonicalize(&plugin.dir).unwrap());
        assert_eq!(second, fs::canonicalize(other.path()).unwrap());
    }

    #[test]
    fn is_applicable_reflects_file_exists() {
        let mut plugin = PluginFixture::test_harness();
        assert!(!plugin.is_applicable());

        fs::write(plugin.dir.join(".starship-test-marker"), "").unwrap();
        assert!(plugin.is_applicable());
    }

    #[test]
    fn general_plugin_reports_general_kind_and_no_shadows() {
        let plugin = PluginFixture::test_harness();
        assert_eq!(plugin.kind(), PluginKind::General);
        assert_eq!(plugin.shadows(), Vec::<String>::new());
    }

    #[test]
    fn vcs_plugin_loads_with_declared_name() {
        let plugin = PluginFixture::vcs_test_harness();
        assert_eq!(plugin.name(), "vcs-test");
    }

    #[test]
    fn vcs_plugin_reports_vcs_kind_and_shadows() {
        let plugin = PluginFixture::vcs_test_harness();
        assert_eq!(plugin.kind(), PluginKind::Vcs);
        assert_eq!(plugin.shadows(), vec!["other-vcs".to_string()]);
    }

    #[test]
    fn vcs_plugin_is_applicable_when_detected() {
        let mut plugin = PluginFixture::vcs_test_harness();
        assert!(!plugin.is_applicable());

        fs::write(plugin.dir.join(".vcs-test-marker"), "").unwrap();
        assert!(plugin.is_applicable());
    }

    #[test]
    fn vcs_plugin_reports_detect_depth() {
        let mut plugin = PluginFixture::vcs_test_harness();
        assert_eq!(plugin.detect_depth(), None);

        fs::write(plugin.dir.join(".vcs-test-marker"), "").unwrap();
        assert_eq!(plugin.detect_depth(), Some(0));
    }

    #[test]
    fn vcs_plugin_routes_root_and_branch_to_trait() {
        let mut plugin = PluginFixture::vcs_test_harness();
        assert_eq!(plugin.get("root").as_deref(), Some("/tmp/vcs-test"));
        assert_eq!(plugin.get("branch").as_deref(), Some("main"));
    }

    #[test]
    fn vcs_plugin_routes_inherent_methods() {
        let mut plugin = PluginFixture::vcs_test_harness();
        assert_eq!(plugin.get("change_id").as_deref(), Some("stub-change-id"));
    }
}
