use std::cell::RefCell;
use std::collections::HashMap;
use std::path::Path;
use std::rc::Rc;

use anyhow::{Context, Result, bail, ensure};
use mlua::{Lua, LuaSerdeExt, Table};
use serde::Serialize;
use serde::de::DeserializeOwned;
use serde_json::Value;
use starship_plugin_core::{
    ABI_VERSION, HostRequest, HostResponse, Manifest, PluginKind, RenderContext, Request, Response,
    from_bitwise, into_bitwise,
};
use tracing::instrument;
use wasmtime::{AsContextMut, Cache, Caller, Engine, Linker, Memory, Module, Store, TypedFunc};

use crate::exec_cache::ExecCache;

/// Creates a wasmtime Engine with disk-backed compilation caching.
///
/// Compiled machine code is persisted to the platform cache directory
/// (e.g. `~/Library/Caches/wasmtime` on macOS). The cache key includes
/// the wasm bytes, engine config, and wasmtime version, so it
/// automatically invalidates when any of these change.
pub fn create_engine() -> Result<Engine> {
    let mut config = wasmtime::Config::new();
    config.cache(Some(Cache::from_file(None)?));
    Ok(Engine::new(&config)?)
}

/// What the host needs to answer a plugin's [`HostRequest`]s.
struct HostState {
    exec_cache: Rc<ExecCache>,
    /// Set right after instantiation, before any request can arrive.
    guest: Option<GuestMemory>,
}

impl HostState {
    fn respond(&self, request: HostRequest) -> HostResponse {
        match request {
            HostRequest::Exec {
                cmd,
                args,
                cwd,
                cached,
            } => HostResponse::Exec(self.exec(&cmd, &args, &cwd, cached)),
            HostRequest::FileExists { path } => HostResponse::FileExists(path.exists()),
        }
    }

    #[instrument(skip(self, args))]
    fn exec(&self, cmd: &str, args: &[String], cwd: &Path, cached: bool) -> Option<String> {
        if cached && let Some(output) = self.exec_cache.get(cmd, args) {
            tracing::debug!("cache hit");
            return Some(output);
        }
        let output = run_command(cmd, args, cwd)?;
        if cached {
            self.exec_cache.insert(cmd, args, output.clone());
        }
        Some(output)
    }
}

fn run_command(cmd: &str, args: &[String], pwd: &Path) -> Option<String> {
    std::process::Command::new(cmd)
        .args(args)
        .current_dir(pwd)
        .output()
        .ok()
        .filter(|o| o.status.success())
        .map(|o| String::from_utf8_lossy(&o.stdout).to_string())
}

/// Moves JSON messages in and out of a plugin's linear memory.
///
/// Messages are passed as a packed `(ptr, len)` `u64`. Whoever receives a
/// message frees it: the host deallocates what it reads, and the guest frees
/// what the host wrote with `alloc`.
#[derive(Clone)]
struct GuestMemory {
    memory: Memory,
    alloc: TypedFunc<u32, u32>,
    dealloc: TypedFunc<u64, ()>,
}

impl GuestMemory {
    fn read<T: DeserializeOwned>(&self, mut store: impl AsContextMut, packed: u64) -> Result<T> {
        let (ptr, len) = from_bitwise(packed);
        let mut bytes = vec![0u8; len as usize];
        self.memory.read(&store, ptr as usize, &mut bytes)?;
        self.dealloc.call(&mut store, packed)?;
        Ok(serde_json::from_slice(&bytes)?)
    }

    fn write<T: Serialize>(&self, mut store: impl AsContextMut, value: &T) -> Result<u64> {
        let bytes = serde_json::to_vec(value)?;
        let len = u32::try_from(bytes.len()).context("message exceeds wasm32 memory")?;
        let ptr = self.alloc.call(&mut store, len)?;
        self.memory.write(&mut store, ptr as usize, &bytes)?;
        Ok(into_bitwise(ptr, len))
    }
}

/// The plugin's single host import: decode a [`HostRequest`], answer it from
/// the current render, and write back the [`HostResponse`].
fn host_handle(mut caller: Caller<'_, HostState>, packed: u64) -> Result<u64> {
    let guest = caller
        .data()
        .guest
        .clone()
        .context("guest memory not initialized")?;
    let request: HostRequest = guest.read(&mut caller, packed)?;
    let response = caller.data().respond(request);
    guest.write(&mut caller, &response)
}

/// A loaded WASM plugin instance backed by wasmtime.
///
/// All communication goes through the plugin's `_plugin_handle` export using
/// the [`Request`]/[`Response`] protocol. The plugin's [`Manifest`] is read
/// once at load, so its name, kind, and methods are plain fields afterward.
pub struct WasmPlugin {
    store: Store<HostState>,
    guest: GuestMemory,
    handle: TypedFunc<u64, u64>,
    manifest: Manifest,
}

impl WasmPlugin {
    /// Compiles and instantiates a plugin from WASM bytes.
    pub fn load(engine: &Engine, wasm_bytes: &[u8], exec_cache: Rc<ExecCache>) -> Result<Self> {
        let module = tracing::info_span!("compile").in_scope(|| Module::new(engine, wasm_bytes))?;
        Self::from_module(&module, exec_cache)
    }

    /// Creates a plugin instance from a pre-compiled module, skipping WASM
    /// compilation. Use when instantiating the same plugin multiple times.
    pub fn from_module(module: &Module, exec_cache: Rc<ExecCache>) -> Result<Self> {
        let engine = module.engine();
        let mut linker = Linker::new(engine);
        linker.func_wrap(
            "env",
            "_host_handle",
            |caller: Caller<'_, HostState>, packed: u64| -> wasmtime::Result<u64> {
                host_handle(caller, packed).map_err(|err| wasmtime::Error::msg(err.to_string()))
            },
        )?;

        let mut store = Store::new(
            engine,
            HostState {
                exec_cache,
                guest: None,
            },
        );
        let instance = tracing::info_span!("instantiate")
            .in_scope(|| linker.instantiate(&mut store, module))?;

        let guest = GuestMemory {
            memory: instance
                .get_memory(&mut store, "memory")
                .context("missing memory export")?,
            alloc: instance.get_typed_func(&mut store, "alloc")?,
            dealloc: instance.get_typed_func(&mut store, "dealloc")?,
        };
        store.data_mut().guest = Some(guest.clone());
        let handle = instance.get_typed_func(&mut store, "_plugin_handle")?;

        let Response::Describe(manifest) =
            exchange(&mut store, &guest, &handle, &Request::Describe)?
        else {
            bail!("plugin answered Describe with an unexpected response");
        };
        ensure!(
            manifest.abi_version == ABI_VERSION,
            "plugin '{}' uses ABI version {}, expected {ABI_VERSION}",
            manifest.name,
            manifest.abi_version,
        );

        Ok(Self {
            store,
            guest,
            handle,
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

    /// Sends one request. Traps, decode failures, and mismatched responses
    /// are logged and become `None` (or the wrong variant, which callers
    /// treat as a failure).
    fn send(&mut self, request: &Request) -> Option<Response> {
        exchange(&mut self.store, &self.guest, &self.handle, request)
            .inspect_err(|error| {
                tracing::error!(plugin = %self.manifest.name, ?request, %error, "plugin request failed");
            })
            .ok()
    }
}

fn exchange(
    store: &mut Store<HostState>,
    guest: &GuestMemory,
    handle: &TypedFunc<u64, u64>,
    request: &Request,
) -> Result<Response> {
    let packed = guest.write(&mut *store, request)?;
    let result = handle.call(&mut *store, packed)?;
    guest.read(&mut *store, result)
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
    plugin: Rc<RefCell<WasmPlugin>>,
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

/// Scans a directory for `.wasm` files and loads each as a plugin.
///
/// Returns an empty vec if the directory doesn't exist. Logs and skips
/// individual plugins that fail to load.
#[instrument(skip(engine, exec_cache))]
pub fn load_plugins(
    engine: &Engine,
    plugin_dir: &Path,
    exec_cache: &Rc<ExecCache>,
) -> Vec<WasmPlugin> {
    let Ok(entries) = std::fs::read_dir(plugin_dir) else {
        return vec![];
    };
    entries
        .filter_map(std::result::Result::ok)
        .filter(|entry| entry.path().extension().is_some_and(|ext| ext == "wasm"))
        .filter_map(|entry| {
            let path = entry.path();
            let bytes = std::fs::read(&path).ok()?;
            let _span = tracing::info_span!(
                "WasmPlugin::load",
                plugin = %path.file_stem().unwrap_or_default().to_string_lossy(),
            )
            .entered();
            WasmPlugin::load(engine, &bytes, Rc::clone(exec_cache))
                .inspect_err(|error| {
                    tracing::error!(path = %path.display(), %error, "failed to load plugin");
                })
                .ok()
        })
        .collect()
}

#[cfg(any(test, feature = "testing"))]
pub mod test_helpers {
    use std::path::{Path, PathBuf};
    use std::rc::Rc;

    use starship_plugin_core::RenderContext;
    use wasmtime::Module;

    use super::{WasmPlugin, create_engine};
    use crate::exec_cache::ExecCache;

    pub const TEST_HARNESS_WASM: &[u8] = include_bytes!(concat!(
        env!("WASM_PLUGIN_DIR"),
        "/starship_plugin_test_harness.wasm"
    ));

    pub const NODEJS_WASM: &[u8] = include_bytes!(concat!(
        env!("WASM_PLUGIN_DIR"),
        "/starship_plugin_nodejs.wasm"
    ));

    pub const VCS_TEST_HARNESS_WASM: &[u8] = include_bytes!(concat!(
        env!("WASM_PLUGIN_DIR"),
        "/starship_plugin_vcs_test_harness.wasm"
    ));

    /// A render context for `pwd` with the test process's environment.
    pub fn render_context(pwd: &Path) -> RenderContext {
        RenderContext {
            pwd: pwd.to_path_buf(),
            env: std::env::vars().collect(),
        }
    }

    /// A loaded plugin, rendering in its own temporary working directory.
    pub struct PluginFixture {
        pub dir: PathBuf,
        plugin: WasmPlugin,
        module: Module,
        _tempdir: tempfile::TempDir,
    }

    impl PluginFixture {
        /// The general test plugin (`test`), which exercises every `Ctx` helper.
        pub fn test_harness() -> Self {
            Self::from_wasm(TEST_HARNESS_WASM)
        }

        /// The stub VCS plugin (`vcs-test`).
        pub fn vcs_test_harness() -> Self {
            Self::from_wasm(VCS_TEST_HARNESS_WASM)
        }

        pub fn from_wasm(bytes: &[u8]) -> Self {
            let dir = tempfile::TempDir::new().expect("tempdir");
            let engine = create_engine().expect("engine should build");
            let module = Module::new(&engine, bytes).expect("plugin should compile");
            let plugin = WasmPlugin::from_module(&module, Rc::new(ExecCache::in_memory()))
                .expect("plugin should load");
            Self {
                dir: dir.path().to_path_buf(),
                plugin,
                module,
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
            let plugin = WasmPlugin::from_module(&self.module, Rc::new(ExecCache::in_memory()))
                .expect("plugin should load");
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
    use std::rc::Rc;

    use mlua::{Lua, LuaOptions, StdLib};
    use starship_plugin_core::{HostRequest, HostResponse};

    use super::test_helpers::{PluginFixture, render_context};
    use super::{HostState, PluginKind, create_engine, load_plugins};
    use crate::exec_cache::ExecCache;

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
    fn host_answers_file_exists_for_absolute_paths() {
        let dir = tempfile::tempdir().unwrap();
        fs::write(dir.path().join("present"), "").unwrap();
        let state = HostState {
            exec_cache: Rc::new(ExecCache::in_memory()),
            guest: None,
        };
        let exists = |name: &str| {
            state.respond(HostRequest::FileExists {
                path: dir.path().join(name),
            })
        };
        assert_eq!(exists("present"), HostResponse::FileExists(true));
        assert_eq!(exists("absent"), HostResponse::FileExists(false));
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
        let engine = create_engine().unwrap();
        let cache = Rc::new(ExecCache::in_memory());
        let plugins = load_plugins(&engine, plugin_dir.path(), &cache);
        assert!(plugins.is_empty());
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
