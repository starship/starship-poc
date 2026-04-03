use std::cell::{Cell, RefCell};
use std::path::{Path, PathBuf};
use std::rc::Rc;

use anyhow::{anyhow, Result};
use mlua::{Lua, LuaSerdeExt, Table};
use serde_json::Value;
use starship_plugin_core::{from_bitwise, into_bitwise};
use tracing::instrument;
use wasmtime::{Caller, Engine, Instance, Linker, Module, Store};

struct HostState {
    pwd: PathBuf,
}

/// A loaded WASM plugin instance backed by wasmtime.
///
/// Each plugin exposes named accessor methods (e.g. `version`, `branch`) that
/// return JSON values across the WASM boundary. The plugin's lifecycle is tied
/// to this struct — dropping it calls `_plugin_drop` in the guest.
pub struct WasmPlugin {
    store: Store<HostState>,
    instance: Instance,
    name: String,
    handle: u32,
    is_active: Cell<Option<bool>>,
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
    let output = std::process::Command::new(&cmd)
        .args(&args)
        .current_dir(&caller.data().pwd)
        .output();
    let result: Option<String> = output
        .ok()
        .filter(|o| o.status.success())
        .map(|o| String::from_utf8_lossy(&o.stdout).to_string());
    let json = serde_json::to_vec(&result)?;
    write_guest_bytes(caller, &json)
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
        "_plugin_host_file_exists",
        |mut caller: Caller<'_, HostState>, packed: u64| -> wasmtime::Result<u32> {
            host_file_exists(&mut caller, packed)
                .map_err(|err| wasmtime::Error::msg(err.to_string()))
        },
    )?;

    Ok(linker)
}

fn read_packed_bytes(
    instance: &Instance,
    store: &mut Store<HostState>,
    packed: u64,
) -> Result<Vec<u8>> {
    let (ptr, len) = from_bitwise(packed);
    let memory = instance
        .get_memory(&mut *store, "memory")
        .ok_or_else(|| anyhow!("missing memory export"))?;
    let mut buf = vec![0u8; len as usize];
    memory.read(&*store, ptr as usize, &mut buf)?;
    Ok(buf)
}

impl WasmPlugin {
    /// Loads a WASM plugin by compiling bytes and instantiating the module.
    ///
    /// Links host functions (`get_env`, `exec`, `file_exists`), instantiates the module,
    /// reads the plugin name, and creates a guest-side instance handle.
    pub fn load(engine: &Engine, wasm_bytes: &[u8], pwd: &Path) -> Result<Self> {
        let module = tracing::info_span!("compile").in_scope(|| Module::new(engine, wasm_bytes))?;
        Self::from_module(&module, pwd)
    }

    /// Creates a plugin instance from a pre-compiled module, skipping WASM
    /// compilation. Use when instantiating the same plugin multiple times.
    pub fn from_module(module: &Module, pwd: &Path) -> Result<Self> {
        let engine = module.engine();
        let linker = create_linker(engine)?;
        let mut store = Store::new(
            engine,
            HostState {
                pwd: pwd.to_path_buf(),
            },
        );
        let instance = tracing::info_span!("instantiate")
            .in_scope(|| linker.instantiate(&mut store, module))?;
        let name_func = instance.get_typed_func::<(), u64>(&mut store, "_plugin_name")?;
        let name_packed = name_func.call(&mut store, ())?;
        let name_bytes = read_packed_bytes(&instance, &mut store, name_packed)?;
        let dealloc = instance.get_typed_func::<u64, ()>(&mut store, "dealloc")?;
        dealloc.call(&mut store, name_packed)?;
        let name: String = serde_json::from_slice(&name_bytes)?;

        let new_func = instance.get_typed_func::<(), u32>(&mut store, "_plugin_new")?;
        let handle = new_func.call(&mut store, ())?;

        Ok(Self {
            store,
            instance,
            name,
            handle,
            is_active: Cell::new(None),
        })
    }

    #[must_use]
    pub fn name(&self) -> &str {
        &self.name
    }

    /// Updates the working directory for host function calls and invalidates
    /// the cached `is_active` result. Called once per render cycle.
    pub fn update_context(&mut self, pwd: &Path) {
        self.store.data_mut().pwd = pwd.to_path_buf();
        self.is_active.set(None);
    }

    #[instrument(skip_all, fields(plugin = %self.name))]
    pub fn is_active(&mut self) -> bool {
        if let Some(cached) = self.is_active.get() {
            return cached;
        }
        let result = self.is_active_uncached();
        self.is_active.set(Some(result));
        result
    }

    fn is_active_uncached(&mut self) -> bool {
        let Ok(func) = self
            .instance
            .get_typed_func::<u32, u64>(&mut self.store, "_plugin_is_active")
        else {
            return true;
        };
        let Ok(packed) = func.call(&mut self.store, self.handle) else {
            return true;
        };
        let (ptr, len) = from_bitwise(packed);
        let Some(memory) = self.instance.get_memory(&mut self.store, "memory") else {
            return true;
        };
        let mut buf = vec![0u8; len as usize];
        if memory.read(&self.store, ptr as usize, &mut buf).is_err() {
            return true;
        }
        if let Ok(dealloc) = self
            .instance
            .get_typed_func::<u64, ()>(&mut self.store, "dealloc")
        {
            let _ = dealloc.call(&mut self.store, packed);
        }
        serde_json::from_slice::<bool>(&buf).unwrap_or(true)
    }

    /// Calls a named accessor method on the plugin via `_plugin_call`.
    ///
    /// Returns `Value::Null` for unknown methods or if the guest traps.
    /// The caller is responsible for converting the JSON value to the desired type.
    #[instrument(skip(self), fields(plugin = %self.name))]
    pub fn call_method(&mut self, method: &str) -> Result<Value> {
        let method_json = serde_json::to_vec(&method)?;

        let alloc = self
            .instance
            .get_typed_func::<u32, u32>(&mut self.store, "alloc")?;
        #[allow(clippy::cast_possible_truncation)]
        let method_len = method_json.len() as u32;
        let ptr = alloc.call(&mut self.store, method_len)?;
        let memory = self
            .instance
            .get_memory(&mut self.store, "memory")
            .ok_or_else(|| anyhow!("missing memory export"))?;
        memory.write(&mut self.store, ptr as usize, &method_json)?;

        let packed_method = into_bitwise(ptr, method_len);
        let call = self
            .instance
            .get_typed_func::<(u32, u64), u64>(&mut self.store, "_plugin_call")?;
        let packed_result = match call.call(&mut self.store, (self.handle, packed_method)) {
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

        let result_bytes = read_packed_bytes(&self.instance, &mut self.store, packed_result)?;
        let dealloc = self
            .instance
            .get_typed_func::<u64, ()>(&mut self.store, "dealloc")?;
        dealloc.call(&mut self.store, packed_result)?;
        Ok(serde_json::from_slice(&result_bytes)?)
    }
}

impl Drop for WasmPlugin {
    fn drop(&mut self) {
        if let Ok(drop_fn) = self
            .instance
            .get_typed_func::<u32, ()>(&mut self.store, "_plugin_drop")
        {
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
            if !plugin.is_active() {
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
#[instrument(skip(engine, pwd))]
pub fn load_plugins(engine: &Engine, plugin_dir: &Path, pwd: &Path) -> Vec<WasmPlugin> {
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
            match WasmPlugin::load(engine, &bytes, pwd) {
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

    use wasmtime::{Engine, Module};

    use super::WasmPlugin;

    pub const TEST_HARNESS_WASM: &[u8] = include_bytes!(concat!(
        env!("WASM_PLUGIN_DIR"),
        "/starship_plugin_test_harness.wasm"
    ));

    pub const NODEJS_WASM: &[u8] = include_bytes!(concat!(
        env!("WASM_PLUGIN_DIR"),
        "/starship_plugin_nodejs.wasm"
    ));

    pub struct PluginFixture {
        pub dir: PathBuf,
        plugin: WasmPlugin,
        module: Module,
        _tempdir: tempfile::TempDir,
    }

    impl PluginFixture {
        pub fn from_wasm(bytes: &[u8]) -> Self {
            let dir = tempfile::TempDir::new().expect("tempdir");
            let path = dir.path().to_path_buf();
            let engine = Engine::default();
            let module = Module::new(&engine, bytes).expect("plugin should compile");
            let plugin = WasmPlugin::from_module(&module, &path).expect("plugin should load");
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

        /// Calls the guest's `_plugin_is_active` export. Returns `true` (fail-open)
        /// if the export is missing, traps, or returns malformed data.
        pub fn is_active(&mut self) -> bool {
            self.plugin.update_context(&self.dir.clone());
            self.plugin.is_active()
        }

        pub fn render(&mut self, lua_expr: &str) -> String {
            use crate::config::{Config, ConfigLoader};
            use starship_common::ShellContext;

            self.plugin.update_context(&self.dir.clone());
            let lua_src = format!(r#"return {{ format = {lua_expr} }}"#);
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
            WasmPlugin::from_module(&self.module, &self.dir).expect("plugin should load")
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
}

#[cfg(test)]
mod tests {
    use std::fs;

    use mlua::{Lua, LuaOptions, StdLib};
    use wasmtime::Engine;

    use super::load_plugins;
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
        assert_eq!(plugin.get("home").is_some(), true);
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
        let engine = Engine::default();
        let plugins = load_plugins(&engine, plugin_dir.path(), dir.path());
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
    fn is_active_reflects_file_exists() {
        let mut plugin = plugin_fixture!();
        assert!(!plugin.is_active());

        fs::write(plugin.dir.join(".starship-test-marker"), "").unwrap();
        assert!(plugin.is_active());
    }
}
