use std::cell::{Cell, RefCell};
use std::path::{Path, PathBuf};
use std::rc::Rc;
use std::sync::Arc;

use anyhow::{anyhow, Result};
use mlua::{Lua, LuaSerdeExt, Table};
use serde_json::Value;
use starship_plugin_core::{from_bitwise, into_bitwise};
use tracing::instrument;
use wasmtime::{Cache, Caller, Engine, Linker, Memory, Module, Store, TypedFunc};

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

struct HostState {
    pwd: PathBuf,
    exec_cache: Arc<ExecCache>,
}

struct GuestExports {
    memory: Memory,
    alloc: TypedFunc<u32, u32>,
    dealloc: TypedFunc<u64, ()>,
    call: TypedFunc<(u32, u64), u64>,
    is_applicable: Option<TypedFunc<u32, u64>>,
    detect_depth: Option<TypedFunc<u32, u64>>,
    kind: Option<TypedFunc<(), u64>>,
    shadows: Option<TypedFunc<(), u64>>,
    drop: Option<TypedFunc<u32, ()>>,
}

/// A loaded WASM plugin instance backed by wasmtime.
///
/// Each plugin exposes named accessor methods (e.g. `version`, `branch`) that
/// return JSON values across the WASM boundary. The plugin's lifecycle is tied
/// to this struct — dropping it calls `_plugin_drop` in the guest.
pub struct WasmPlugin {
    store: Store<HostState>,
    exports: GuestExports,
    name: String,
    handle: u32,
    is_applicable: Cell<Option<bool>>,
}

fn caller_memory(caller: &mut Caller<'_, HostState>) -> Result<wasmtime::Memory> {
    caller
        .get_export("memory")
        .and_then(wasmtime::Extern::into_memory)
        .ok_or_else(|| anyhow!("missing memory export"))
}

fn caller_alloc(caller: &mut Caller<'_, HostState>, len: u32) -> Result<u32> {
    let alloc = caller
        .get_export("alloc")
        .and_then(wasmtime::Extern::into_func)
        .ok_or_else(|| anyhow!("missing alloc export"))?;
    let alloc = alloc.typed::<u32, u32>(&mut *caller)?;
    Ok(alloc.call(&mut *caller, len)?)
}

fn caller_dealloc(caller: &mut Caller<'_, HostState>, packed: u64) -> Result<()> {
    let dealloc = caller
        .get_export("dealloc")
        .and_then(wasmtime::Extern::into_func)
        .ok_or_else(|| anyhow!("missing dealloc export"))?;
    let dealloc = dealloc.typed::<u64, ()>(&mut *caller)?;
    Ok(dealloc.call(&mut *caller, packed)?)
}

fn read_guest_bytes(caller: &mut Caller<'_, HostState>, packed: u64) -> Result<Vec<u8>> {
    let (ptr, len) = from_bitwise(packed);
    let memory = caller_memory(caller)?;
    let mut buf = vec![0u8; len as usize];
    memory.read(&*caller, ptr as usize, &mut buf)?;
    Ok(buf)
}

fn write_guest_bytes(caller: &mut Caller<'_, HostState>, bytes: &[u8]) -> Result<u64> {
    #[allow(clippy::cast_possible_truncation)]
    let len = bytes.len() as u32;
    let ptr = caller_alloc(caller, len)?;
    let memory = caller_memory(caller)?;
    memory.write(&mut *caller, ptr as usize, bytes)?;
    Ok(into_bitwise(ptr, len))
}

#[instrument(skip_all)]
fn host_get_env(caller: &mut Caller<'_, HostState>, packed: u64) -> Result<u64> {
    let bytes = read_guest_bytes(caller, packed)?;
    caller_dealloc(caller, packed)?;
    let name: String = serde_json::from_slice(&bytes)?;
    let result: Option<String> = std::env::var(&name).ok();
    let json = serde_json::to_vec(&result)?;
    write_guest_bytes(caller, &json)
}

fn host_exec(caller: &mut Caller<'_, HostState>, packed: u64) -> Result<u64> {
    let bytes = read_guest_bytes(caller, packed)?;
    caller_dealloc(caller, packed)?;
    let (cmd, args): (String, Vec<String>) = serde_json::from_slice(&bytes)?;
    let _span = tracing::info_span!("host_exec", %cmd).entered();

    if let Some(cached) = caller.data().exec_cache.get(&cmd, &args) {
        tracing::debug!("cache hit");
        let result: Option<String> = Some(cached);
        let json = serde_json::to_vec(&result)?;
        return write_guest_bytes(caller, &json);
    }

    let result = run_command(&cmd, &args, &caller.data().pwd);

    if let Some(ref output) = result {
        caller.data().exec_cache.insert(&cmd, &args, output.clone());
    }

    let json = serde_json::to_vec(&result)?;
    write_guest_bytes(caller, &json)
}

fn host_exec_uncached(caller: &mut Caller<'_, HostState>, packed: u64) -> Result<u64> {
    let bytes = read_guest_bytes(caller, packed)?;
    caller_dealloc(caller, packed)?;
    let (cmd, args): (String, Vec<String>) = serde_json::from_slice(&bytes)?;
    let _span = tracing::info_span!("host_exec_uncached", %cmd).entered();
    let result = run_command(&cmd, &args, &caller.data().pwd);
    let json = serde_json::to_vec(&result)?;
    write_guest_bytes(caller, &json)
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

#[instrument(skip_all)]
fn host_file_exists(caller: &mut Caller<'_, HostState>, packed: u64) -> Result<u32> {
    let bytes = read_guest_bytes(caller, packed)?;
    caller_dealloc(caller, packed)?;
    let path: String = serde_json::from_slice(&bytes)?;
    let full_path = caller.data().pwd.join(path);
    Ok(u32::from(full_path.exists()))
}

fn create_linker(engine: &Engine) -> Result<Linker<HostState>> {
    let mut linker = Linker::new(engine);

    linker.func_wrap(
        "env",
        "_plugin_host_get_env",
        |mut caller: Caller<'_, HostState>, packed: u64| -> wasmtime::Result<u64> {
            host_get_env(&mut caller, packed).map_err(|err| wasmtime::Error::msg(err.to_string()))
        },
    )?;

    linker.func_wrap(
        "env",
        "_plugin_host_exec",
        |mut caller: Caller<'_, HostState>, packed: u64| -> wasmtime::Result<u64> {
            host_exec(&mut caller, packed).map_err(|err| wasmtime::Error::msg(err.to_string()))
        },
    )?;

    linker.func_wrap(
        "env",
        "_plugin_host_exec_uncached",
        |mut caller: Caller<'_, HostState>, packed: u64| -> wasmtime::Result<u64> {
            host_exec_uncached(&mut caller, packed)
                .map_err(|err| wasmtime::Error::msg(err.to_string()))
        },
    )?;

    linker.func_wrap(
        "env",
        "_plugin_host_file_exists",
        |mut caller: Caller<'_, HostState>, packed: u64| -> wasmtime::Result<u32> {
            host_file_exists(&mut caller, packed)
                .map_err(|err| wasmtime::Error::msg(err.to_string()))
        },
    )?;

    Ok(linker)
}

impl WasmPlugin {
    /// Loads a WASM plugin by compiling bytes and instantiating the module.
    ///
    /// Links host functions (`get_env`, `exec`, `exec_uncached`, `file_exists`),
    /// instantiates the module, reads the plugin name, and creates a guest-side
    /// instance handle.
    pub fn load(
        engine: &Engine,
        wasm_bytes: &[u8],
        pwd: &Path,
        exec_cache: Arc<ExecCache>,
    ) -> Result<Self> {
        let module = tracing::info_span!("compile").in_scope(|| Module::new(engine, wasm_bytes))?;
        Self::from_module(&module, pwd, exec_cache)
    }

    /// Creates a plugin instance from a pre-compiled module, skipping WASM
    /// compilation. Use when instantiating the same plugin multiple times.
    pub fn from_module(module: &Module, pwd: &Path, exec_cache: Arc<ExecCache>) -> Result<Self> {
        let engine = module.engine();
        let linker = create_linker(engine)?;
        let mut store = Store::new(
            engine,
            HostState {
                pwd: pwd.to_path_buf(),
                exec_cache,
            },
        );
        let instance = tracing::info_span!("instantiate")
            .in_scope(|| linker.instantiate(&mut store, module))?;

        let exports = GuestExports {
            memory: instance
                .get_memory(&mut store, "memory")
                .ok_or_else(|| anyhow!("missing memory export"))?,
            alloc: instance.get_typed_func(&mut store, "alloc")?,
            dealloc: instance.get_typed_func(&mut store, "dealloc")?,
            call: instance.get_typed_func(&mut store, "_plugin_call")?,
            is_applicable: instance
                .get_typed_func(&mut store, "_plugin_is_applicable")
                .ok(),
            detect_depth: instance
                .get_typed_func(&mut store, "_plugin_detect_depth")
                .ok(),
            kind: instance.get_typed_func(&mut store, "_plugin_kind").ok(),
            shadows: instance.get_typed_func(&mut store, "_plugin_shadows").ok(),
            drop: instance.get_typed_func(&mut store, "_plugin_drop").ok(),
        };

        let name_func = instance.get_typed_func::<(), u64>(&mut store, "_plugin_name")?;
        let name_packed = name_func.call(&mut store, ())?;
        let name_bytes = Self::read_guest_bytes_raw(&exports, &store, name_packed)?;
        exports.dealloc.call(&mut store, name_packed)?;
        let name: String = serde_json::from_slice(&name_bytes)?;

        let new_func = instance.get_typed_func::<(), u32>(&mut store, "_plugin_new")?;
        let handle = new_func.call(&mut store, ())?;

        Ok(Self {
            store,
            exports,
            name,
            handle,
            is_applicable: Cell::new(None),
        })
    }

    #[must_use]
    pub fn name(&self) -> &str {
        &self.name
    }

    /// Reads the plugin's `_plugin_kind` export, falling back to `"general"`
    /// when the export is missing (older plugins built before kind classification).
    pub fn kind(&mut self) -> String {
        let Some(func) = self.exports.kind.clone() else {
            return "general".to_string();
        };
        let Ok(packed) = func.call(&mut self.store, ()) else {
            return "general".to_string();
        };
        let Ok(bytes) = self.read_guest_bytes(packed) else {
            return "general".to_string();
        };
        let _ = self.exports.dealloc.call(&mut self.store, packed);
        serde_json::from_slice::<String>(&bytes).unwrap_or_else(|_| "general".to_string())
    }

    /// Reads the plugin's `_plugin_shadows` export, returning the declared
    /// list of shadowed plugin names. Empty for plugins missing the export
    /// or for general (non-VCS) plugins.
    pub fn shadows(&mut self) -> Vec<String> {
        let Some(func) = self.exports.shadows.clone() else {
            return Vec::new();
        };
        let Ok(packed) = func.call(&mut self.store, ()) else {
            return Vec::new();
        };
        let Ok(bytes) = self.read_guest_bytes(packed) else {
            return Vec::new();
        };
        let _ = self.exports.dealloc.call(&mut self.store, packed);
        serde_json::from_slice::<Vec<String>>(&bytes).unwrap_or_default()
    }

    /// Reads the plugin's `_plugin_detect_depth` export. Returns `None` for
    /// plugins missing the export (general plugins, or VCS plugins that
    /// declined to detect at the current pwd).
    pub fn detect_depth(&mut self) -> Option<u32> {
        let func = self.exports.detect_depth.clone()?;
        let packed = func.call(&mut self.store, self.handle).ok()?;
        let bytes = self.read_guest_bytes(packed).ok()?;
        let _ = self.exports.dealloc.call(&mut self.store, packed);
        serde_json::from_slice::<Option<u32>>(&bytes).ok().flatten()
    }

    /// Updates the working directory for host function calls and invalidates
    /// the cached `is_applicable` result. Called once per render cycle.
    pub fn update_context(&mut self, pwd: &Path) {
        self.store.data_mut().pwd = pwd.to_path_buf();
        self.is_applicable.set(None);
    }

    #[instrument(skip_all, fields(plugin = %self.name))]
    pub fn is_applicable(&mut self) -> bool {
        if let Some(cached) = self.is_applicable.get() {
            return cached;
        }
        let result = self.is_applicable_uncached();
        self.is_applicable.set(Some(result));
        result
    }

    fn is_applicable_uncached(&mut self) -> bool {
        let Some(func) = self.exports.is_applicable.clone() else {
            return true;
        };
        let Ok(packed) = func.call(&mut self.store, self.handle) else {
            return true;
        };
        let Ok(bytes) = self.read_guest_bytes(packed) else {
            return true;
        };
        let _ = self.exports.dealloc.call(&mut self.store, packed);
        serde_json::from_slice::<bool>(&bytes).unwrap_or(true)
    }

    /// Calls a named accessor method on the plugin via `_plugin_call`.
    ///
    /// Returns `Value::Null` for unknown methods or if the guest traps.
    /// The caller is responsible for converting the JSON value to the desired type.
    #[instrument(skip(self), fields(plugin = %self.name))]
    pub fn call_method(&mut self, method: &str) -> Result<Value> {
        let packed_method = self.write_guest_bytes(&serde_json::to_vec(&method)?)?;
        let packed_result = match self
            .exports
            .call
            .call(&mut self.store, (self.handle, packed_method))
        {
            Ok(value) => value,
            Err(err) => {
                tracing::error!(
                    "Plugin '{}' trapped on method '{}': {}",
                    self.name,
                    method,
                    err
                );
                return Ok(Value::Null);
            }
        };

        let result_bytes = self.read_guest_bytes(packed_result)?;
        self.exports.dealloc.call(&mut self.store, packed_result)?;
        Ok(serde_json::from_slice(&result_bytes)?)
    }

    fn read_guest_bytes(&self, packed: u64) -> Result<Vec<u8>> {
        Self::read_guest_bytes_raw(&self.exports, &self.store, packed)
    }

    fn read_guest_bytes_raw(
        exports: &GuestExports,
        store: &Store<HostState>,
        packed: u64,
    ) -> Result<Vec<u8>> {
        let (ptr, len) = from_bitwise(packed);
        let mut buf = vec![0u8; len as usize];
        exports.memory.read(store, ptr as usize, &mut buf)?;
        Ok(buf)
    }

    fn write_guest_bytes(&mut self, bytes: &[u8]) -> Result<u64> {
        #[allow(clippy::cast_possible_truncation)]
        let len = bytes.len() as u32;
        let ptr = self.exports.alloc.call(&mut self.store, len)?;
        self.exports
            .memory
            .write(&mut self.store, ptr as usize, bytes)?;
        Ok(into_bitwise(ptr, len))
    }
}

impl Drop for WasmPlugin {
    fn drop(&mut self) {
        if let Some(drop_fn) = self.exports.drop.clone() {
            let _ = drop_fn.call(&mut self.store, self.handle);
        }
    }
}

/// Registers a plugin as a Lua global with an `__index` metamethod.
///
/// Accessing `plugin_name.field` in Lua triggers `_plugin_call` via wasmtime.
/// Skips registration (with a warning) if the name collides with an existing global.
pub fn register_plugin(lua: &Lua, plugin: Rc<RefCell<WasmPlugin>>) -> mlua::Result<()> {
    let name = plugin.borrow().name().to_string();

    if lua.globals().contains_key(name.as_str())? {
        tracing::warn!(
            "Plugin '{}' name collides with existing Lua global, skipping",
            name
        );
        return Ok(());
    }

    let proxy: Table = lua.create_table()?;
    let meta: Table = lua.create_table()?;

    meta.set(
        "__index",
        lua.create_function(move |lua, (_table, key): (Table, String)| {
            let mut plugin = plugin.borrow_mut();
            if !plugin.is_applicable() {
                return Ok(mlua::Value::Nil);
            }
            let result = plugin
                .call_method(&key)
                .map_err(|e| mlua::Error::RuntimeError(e.to_string()))?;
            lua.to_value(&result)
                .map_err(|e| mlua::Error::RuntimeError(e.to_string()))
        })?,
    )?;

    proxy.set_metatable(Some(meta))?;
    lua.globals().set(name.as_str(), proxy)?;
    Ok(())
}

/// Scans a directory for `.wasm` files and loads each as a plugin.
///
/// Returns an empty vec if the directory doesn't exist. Logs and skips
/// individual plugins that fail to load.
#[must_use]
#[instrument(skip(engine, pwd, exec_cache))]
pub fn load_plugins(
    engine: &Engine,
    plugin_dir: &Path,
    pwd: &Path,
    exec_cache: &Arc<ExecCache>,
) -> Vec<WasmPlugin> {
    if !plugin_dir.exists() {
        return vec![];
    }
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
            match WasmPlugin::load(engine, &bytes, pwd, Arc::clone(exec_cache)) {
                Ok(plugin) => Some(plugin),
                Err(err) => {
                    tracing::error!("Failed to load plugin {}: {}", path.display(), err);
                    None
                }
            }
        })
        .collect()
}

#[cfg(any(test, feature = "testing"))]
pub mod test_helpers {
    use std::path::PathBuf;
    use std::sync::Arc;

    use wasmtime::Module;

    use super::{create_engine, WasmPlugin};
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

    pub struct PluginFixture {
        pub dir: PathBuf,
        plugin: WasmPlugin,
        module: Module,
        _tempdir: tempfile::TempDir,
    }

    impl PluginFixture {
        #[must_use]
        #[allow(clippy::missing_panics_doc)]
        pub fn from_wasm(bytes: &[u8]) -> Self {
            let dir = tempfile::TempDir::new().expect("tempdir");
            let path = dir.path().to_path_buf();
            let engine = create_engine().expect("engine should build");
            let module = Module::new(&engine, bytes).expect("plugin should compile");
            let cache = Arc::new(ExecCache::in_memory());
            let plugin =
                WasmPlugin::from_module(&module, &path, cache).expect("plugin should load");
            Self {
                dir: path,
                plugin,
                module,
                _tempdir: dir,
            }
        }

        pub fn get(&mut self, field: &str) -> Option<String> {
            self.plugin.update_context(&self.dir.clone());
            let value = self.plugin.call_method(field).ok()?;
            match value {
                serde_json::Value::Null => None,
                serde_json::Value::String(s) => Some(s),
                other => Some(other.to_string()),
            }
        }

        /// Calls the guest's `_plugin_is_applicable` export. Returns `true` (fail-open)
        /// if the export is missing, traps, or returns malformed data.
        pub fn is_applicable(&mut self) -> bool {
            self.plugin.update_context(&self.dir.clone());
            self.plugin.is_applicable()
        }

        pub fn kind(&mut self) -> String {
            self.plugin.kind()
        }

        pub fn shadows(&mut self) -> Vec<String> {
            self.plugin.shadows()
        }

        pub fn detect_depth(&mut self) -> Option<u32> {
            self.plugin.update_context(&self.dir.clone());
            self.plugin.detect_depth()
        }

        #[must_use]
        pub fn name(&self) -> &str {
            self.plugin.name()
        }

        #[allow(clippy::missing_panics_doc)]
        pub fn render(&mut self, lua_expr: &str) -> String {
            use crate::config::{Config, ConfigLoader};
            use starship_common::ShellContext;

            self.plugin.update_context(&self.dir.clone());
            let lua_src = format!(r"return {{ format = {lua_expr} }}");
            let mut loader =
                ConfigLoader::from_source_with_plugins(&lua_src, vec![self.plugin_for_loader()])
                    .expect("loader should build");
            let ctx = ShellContext {
                pwd: Some(self.dir.clone()),
                user: Some("test".into()),
            };
            let func = loader.load(&ctx).expect("config should load");
            let output: Config = func.call(()).expect("lua should evaluate");
            output.format.to_string()
        }

        fn plugin_for_loader(&self) -> WasmPlugin {
            let cache = Arc::new(ExecCache::in_memory());
            WasmPlugin::from_module(&self.module, &self.dir, cache).expect("plugin should load")
        }
    }

    #[macro_export]
    macro_rules! plugin_fixture {
        () => {
            $crate::plugin::test_helpers::PluginFixture::from_wasm(
                $crate::plugin::test_helpers::TEST_HARNESS_WASM,
            )
        };
    }

    #[macro_export]
    macro_rules! vcs_plugin_fixture {
        () => {
            $crate::plugin::test_helpers::PluginFixture::from_wasm(
                $crate::plugin::test_helpers::VCS_TEST_HARNESS_WASM,
            )
        };
    }
}

#[cfg(test)]
mod tests {
    use std::fs;
    use std::sync::Arc;

    use mlua::{Lua, LuaOptions, StdLib};

    use super::{create_engine, load_plugins};
    use crate::exec_cache::ExecCache;
    use crate::plugin_fixture;

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
    fn plugin_loads_and_returns_name() {
        let mut plugin = plugin_fixture!();
        assert!(plugin.get("home").is_some());
    }

    #[test]
    fn unknown_method_returns_null() {
        let mut plugin = plugin_fixture!();
        assert!(plugin.get("does_not_exist").is_none());
    }

    #[test]
    fn load_plugins_empty_dir_returns_empty_vec() {
        let dir = tempfile::tempdir().expect("tempdir");
        let plugin_dir = tempfile::tempdir().expect("plugin dir");
        let engine = create_engine().unwrap();
        let cache = Arc::new(ExecCache::in_memory());
        let plugins = load_plugins(&engine, plugin_dir.path(), dir.path(), &cache);
        assert!(plugins.is_empty());
    }

    #[test]
    fn host_get_env() {
        let mut plugin = plugin_fixture!();
        assert!(plugin.get("home").is_some());
    }

    #[test]
    fn host_exec() {
        let mut plugin = plugin_fixture!();
        let pwd = plugin.get("pwd").expect("pwd should return a string");
        let actual = std::fs::canonicalize(&pwd).expect("pwd output resolves");
        let expected = std::fs::canonicalize(&plugin.dir).expect("tempdir path resolves");
        assert_eq!(actual, expected);
    }

    #[test]
    fn is_applicable_reflects_file_exists() {
        let mut plugin = plugin_fixture!();
        assert!(!plugin.is_applicable());

        fs::write(plugin.dir.join(".starship-test-marker"), "").unwrap();
        assert!(plugin.is_applicable());
    }

    #[test]
    fn export_plugin_emits_general_kind() {
        let mut plugin = plugin_fixture!();
        assert_eq!(plugin.kind(), "general");
    }

    #[test]
    fn export_plugin_emits_empty_shadows() {
        let mut plugin = plugin_fixture!();
        assert!(plugin.shadows().is_empty());
    }

    #[test]
    fn vcs_plugin_loads_with_declared_name() {
        let plugin = crate::vcs_plugin_fixture!();
        assert_eq!(plugin.name(), "vcs-test");
    }

    #[test]
    fn export_vcs_plugin_emits_vcs_kind() {
        let mut plugin = crate::vcs_plugin_fixture!();
        assert_eq!(plugin.kind(), "vcs");
    }

    #[test]
    fn export_vcs_plugin_emits_declared_shadows() {
        let mut plugin = crate::vcs_plugin_fixture!();
        assert_eq!(plugin.shadows(), vec!["other-vcs".to_string()]);
    }

    #[test]
    fn export_vcs_plugin_synthesizes_is_applicable_from_detect_depth() {
        let mut plugin = crate::vcs_plugin_fixture!();
        assert!(!plugin.is_applicable());

        fs::write(plugin.dir.join(".vcs-test-marker"), "").unwrap();
        assert!(plugin.is_applicable());
    }

    #[test]
    fn export_vcs_plugin_emits_detect_depth_export() {
        let mut plugin = crate::vcs_plugin_fixture!();
        assert_eq!(plugin.detect_depth(), None);

        fs::write(plugin.dir.join(".vcs-test-marker"), "").unwrap();
        assert_eq!(plugin.detect_depth(), Some(0));
    }

    #[test]
    fn export_vcs_plugin_call_routes_root_and_branch_to_trait() {
        let mut plugin = crate::vcs_plugin_fixture!();
        assert_eq!(plugin.get("root").as_deref(), Some("/tmp/vcs-test"));
        assert_eq!(plugin.get("branch").as_deref(), Some("main"));
    }

    #[test]
    fn export_vcs_plugin_call_routes_inherent_methods() {
        let mut plugin = crate::vcs_plugin_fixture!();
        assert_eq!(plugin.get("change_id").as_deref(), Some("stub-change-id"));
    }
}
