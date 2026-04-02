use std::cell::RefCell;
use std::path::{Path, PathBuf};
use std::rc::Rc;

use anyhow::{anyhow, Result};
use mlua::{Lua, LuaSerdeExt, Table};
use serde_json::Value;
use starship_plugin_core::{from_bitwise, into_bitwise};
use wasmtime::{Caller, Engine, Instance, Linker, Module, Store};

struct HostState {
    pwd: PathBuf,
}

pub struct WasmPlugin {
    store: Store<HostState>,
    instance: Instance,
    name: String,
    handle: u32,
}

fn caller_memory(caller: &mut Caller<'_, HostState>) -> Result<wasmtime::Memory> {
    caller
        .get_export("memory")
        .and_then(|export| export.into_memory())
        .ok_or_else(|| anyhow!("missing memory export"))
}

fn caller_alloc(caller: &mut Caller<'_, HostState>, len: u32) -> Result<u32> {
    let alloc = caller
        .get_export("alloc")
        .and_then(|export| export.into_func())
        .ok_or_else(|| anyhow!("missing alloc export"))?;
    let alloc = alloc.typed::<u32, u32>(&mut *caller)?;
    Ok(alloc.call(&mut *caller, len)?)
}

fn caller_dealloc(caller: &mut Caller<'_, HostState>, packed: u64) -> Result<()> {
    let dealloc = caller
        .get_export("dealloc")
        .and_then(|export| export.into_func())
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
    let ptr = caller_alloc(caller, bytes.len() as u32)?;
    let memory = caller_memory(caller)?;
    memory.write(&mut *caller, ptr as usize, bytes)?;
    Ok(into_bitwise(ptr, bytes.len() as u32))
}

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
    let pwd = caller.data().pwd.clone();
    let output = std::process::Command::new(&cmd)
        .args(&args)
        .current_dir(pwd)
        .output();
    let result: Option<String> = output
        .ok()
        .filter(|o| o.status.success())
        .map(|o| String::from_utf8_lossy(&o.stdout).to_string());
    let json = serde_json::to_vec(&result)?;
    write_guest_bytes(caller, &json)
}

fn host_file_exists(caller: &mut Caller<'_, HostState>, packed: u64) -> Result<u32> {
    let bytes = read_guest_bytes(caller, packed)?;
    caller_dealloc(caller, packed)?;
    let path: String = serde_json::from_slice(&bytes)?;
    let full_path = caller.data().pwd.join(path);
    Ok(if full_path.exists() { 1 } else { 0 })
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
    pub fn load(engine: &Engine, wasm_bytes: &[u8], pwd: &Path) -> Result<Self> {
        let module = Module::new(engine, wasm_bytes)?;
        let linker = create_linker(engine)?;
        let mut store = Store::new(
            engine,
            HostState {
                pwd: pwd.to_path_buf(),
            },
        );
        let instance = linker.instantiate(&mut store, &module)?;
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
        })
    }

    pub fn name(&self) -> &str {
        &self.name
    }

    pub fn update_context(&mut self, pwd: &Path) {
        self.store.data_mut().pwd = pwd.to_path_buf();
    }

    pub fn is_active(&mut self) -> bool {
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
        let memory = match self.instance.get_memory(&mut self.store, "memory") {
            Some(m) => m,
            None => return true,
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

    pub fn call_method(&mut self, method: &str) -> Result<Value> {
        let method_json = serde_json::to_vec(&method)?;

        let alloc = self
            .instance
            .get_typed_func::<u32, u32>(&mut self.store, "alloc")?;
        let ptr = alloc.call(&mut self.store, method_json.len() as u32)?;
        let memory = self
            .instance
            .get_memory(&mut self.store, "memory")
            .ok_or_else(|| anyhow!("missing memory export"))?;
        memory.write(&mut self.store, ptr as usize, &method_json)?;

        let packed_method = into_bitwise(ptr, method_json.len() as u32);
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

pub fn load_plugins(engine: &Engine, plugin_dir: &Path, pwd: &Path) -> Vec<WasmPlugin> {
    if !plugin_dir.exists() {
        return vec![];
    }
    let Ok(entries) = std::fs::read_dir(plugin_dir) else {
        return vec![];
    };
    entries
        .filter_map(|entry| entry.ok())
        .filter(|entry| entry.path().extension().is_some_and(|ext| ext == "wasm"))
        .filter_map(|entry| {
            let path = entry.path();
            let bytes = std::fs::read(&path).ok()?;
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

#[cfg(test)]
mod tests {
    use std::path::Path;

    use mlua::{Lua, LuaOptions, StdLib};
    use serde_json::Value;
    use tempfile::tempdir;
    use wasmtime::Engine;

    use super::{load_plugins, WasmPlugin};

    fn wasm_path(name: &str) -> std::path::PathBuf {
        Path::new(env!("CARGO_MANIFEST_DIR"))
            .parent()
            .and_then(|path| path.parent())
            .unwrap_or_else(|| Path::new("/"))
            .join(format!("target/wasm32-unknown-unknown/release/{name}.wasm"))
    }

    fn load_test_plugin(pwd: &Path) -> WasmPlugin {
        let bytes = std::fs::read(wasm_path("starship_plugin_test_harness"))
            .expect("test-harness.wasm should exist (built by build.rs)");
        let engine = Engine::default();
        WasmPlugin::load(&engine, &bytes, pwd).expect("test plugin should load")
    }

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
        let dir = tempdir().expect("tempdir");
        let plugin = load_test_plugin(dir.path());
        assert_eq!(plugin.name(), "test");
    }

    #[test]
    fn unknown_method_returns_null() {
        let dir = tempdir().expect("tempdir");
        let mut plugin = load_test_plugin(dir.path());
        let result = plugin
            .call_method("does_not_exist")
            .expect("call_method should return a value");
        assert_eq!(result, Value::Null);
    }

    #[test]
    fn load_plugins_empty_dir_returns_empty_vec() {
        let dir = tempdir().expect("tempdir");
        let plugin_dir = tempdir().expect("plugin dir");
        let engine = Engine::default();
        let plugins = load_plugins(&engine, plugin_dir.path(), dir.path());
        assert!(plugins.is_empty());
    }

    #[test]
    fn host_get_env() {
        let dir = tempdir().expect("tempdir");
        let mut plugin = load_test_plugin(dir.path());
        let result = plugin
            .call_method("home")
            .expect("call_method should succeed");
        assert!(result.is_string(), "HOME should be a string, got: {result}");
    }

    #[test]
    fn host_exec() {
        let dir = tempdir().expect("tempdir should be created");
        let mut plugin = load_test_plugin(dir.path());
        let result = plugin
            .call_method("pwd")
            .expect("call_method should succeed");
        let output = result.as_str().expect("pwd should return a string");
        let actual = std::fs::canonicalize(output).expect("pwd output resolves");
        let expected = std::fs::canonicalize(dir.path()).expect("tempdir path resolves");
        assert_eq!(actual, expected);
    }

    #[test]
    fn is_active_reflects_file_exists() {
        let dir = tempdir().expect("tempdir should be created");

        let mut plugin = load_test_plugin(dir.path());
        assert!(!plugin.is_active(), "no marker file = inactive");

        std::fs::write(dir.path().join(".starship-test-marker"), "").unwrap();
        assert!(plugin.is_active(), "marker file present = active");
    }
}
