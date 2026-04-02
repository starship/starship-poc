use std::cell::RefCell;
use std::path::{Path, PathBuf};
use std::rc::Rc;

use anyhow::{anyhow, Result};
use mlua::{Lua, LuaSerdeExt, Table};
use serde::de::DeserializeOwned;
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
        .map(|value| String::from_utf8_lossy(&value.stdout).to_string());
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

fn read_packed_json<T: DeserializeOwned>(
    instance: &Instance,
    store: &mut Store<HostState>,
    packed: u64,
) -> Result<T> {
    let bytes = read_packed_bytes(instance, store, packed)?;
    let dealloc = instance.get_typed_func::<u64, ()>(&mut *store, "dealloc")?;
    dealloc.call(&mut *store, packed)?;
    Ok(serde_json::from_slice(&bytes)?)
}

fn read_packed_string(
    instance: &Instance,
    store: &mut Store<HostState>,
    func_name: &str,
) -> Result<String> {
    let func = instance.get_typed_func::<(), u64>(&mut *store, func_name)?;
    let packed = func.call(&mut *store, ())?;
    read_packed_json(instance, store, packed)
}

fn create_plugin_handle(instance: &Instance, store: &mut Store<HostState>) -> Result<u32> {
    let func = instance.get_typed_func::<(), u32>(&mut *store, "_plugin_new")?;
    Ok(func.call(&mut *store, ())?)
}

fn read_plugin_name(instance: &Instance, store: &mut Store<HostState>) -> Result<String> {
    read_packed_string(instance, store, "_plugin_name")
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
        let name = read_plugin_name(&instance, &mut store)?;
        let handle = create_plugin_handle(&instance, &mut store)?;

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
            let result = plugin
                .borrow_mut()
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
    use std::path::{Path, PathBuf};

    use mlua::{Lua, LuaOptions, StdLib};
    use serde::de::DeserializeOwned;
    use serde::Serialize;
    use serde_json::Value;
    use tempfile::tempdir;
    use wasmtime::{Engine, Instance, Module, Store};

    use super::{create_linker, load_plugins, HostState, WasmPlugin};
    use starship_plugin_core::{from_bitwise, into_bitwise};

    const HOST_TEST_WAT: &str = r#"(module
      (import "env" "_plugin_host_get_env" (func $host_get_env (param i64) (result i64)))
      (import "env" "_plugin_host_exec" (func $host_exec (param i64) (result i64)))
      (import "env" "_plugin_host_file_exists" (func $host_file_exists (param i64) (result i32)))
      (memory (export "memory") 1)
      (global $heap (mut i32) (i32.const 1024))

      (func (export "alloc") (param $len i32) (result i32)
        (local $ptr i32)
        global.get $heap
        local.tee $ptr
        local.get $len
        i32.add
        global.set $heap
        local.get $ptr
      )

      (func (export "dealloc") (param i64))

      (func (export "call_get_env") (param i64) (result i64)
        local.get 0
        call $host_get_env
      )

      (func (export "call_exec") (param i64) (result i64)
        local.get 0
        call $host_exec
      )

      (func (export "call_file_exists") (param i64) (result i32)
        local.get 0
        call $host_file_exists
      )
    )"#;

    fn nodejs_wasm_path() -> PathBuf {
        Path::new(env!("CARGO_MANIFEST_DIR"))
            .parent()
            .and_then(|path| path.parent())
            .unwrap_or_else(|| Path::new("/"))
            .join("target/wasm32-unknown-unknown/release/nodejs.wasm")
    }

    fn instantiate_host_test_module(engine: &Engine, pwd: &Path) -> (Store<HostState>, Instance) {
        let module = Module::new(engine, HOST_TEST_WAT).expect("host test module compiles");
        let linker = create_linker(engine).expect("linker compiles");
        let mut store = Store::new(
            engine,
            HostState {
                pwd: pwd.to_path_buf(),
            },
        );
        let instance = linker
            .instantiate(&mut store, &module)
            .expect("host test module instantiates");
        (store, instance)
    }

    fn write_json<T: Serialize>(
        instance: &Instance,
        store: &mut Store<HostState>,
        value: &T,
    ) -> u64 {
        let bytes = serde_json::to_vec(value).expect("json serializes");
        let alloc = instance
            .get_typed_func::<u32, u32>(&mut *store, "alloc")
            .expect("alloc export exists");
        let ptr = alloc
            .call(&mut *store, bytes.len() as u32)
            .expect("alloc succeeds");
        let memory = instance
            .get_memory(&mut *store, "memory")
            .expect("memory export exists");
        memory
            .write(&mut *store, ptr as usize, &bytes)
            .expect("memory write succeeds");
        into_bitwise(ptr, bytes.len() as u32)
    }

    fn read_json<T: DeserializeOwned>(
        instance: &Instance,
        store: &mut Store<HostState>,
        packed: u64,
    ) -> T {
        let (ptr, len) = from_bitwise(packed);
        let ptr = ptr as usize;
        let len = len as usize;
        let memory = instance
            .get_memory(&mut *store, "memory")
            .expect("memory export exists");
        let bytes = memory.data(&*store)[ptr..(ptr + len)].to_vec();
        let dealloc = instance
            .get_typed_func::<u64, ()>(&mut *store, "dealloc")
            .expect("dealloc export exists");
        dealloc.call(&mut *store, packed).expect("dealloc succeeds");
        serde_json::from_slice(&bytes).expect("json deserializes")
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
    fn wasm_plugin_loads_and_returns_name() {
        let wasm_path = nodejs_wasm_path();
        if !wasm_path.exists() {
            eprintln!("Skipping: nodejs.wasm not found at {:?}", wasm_path);
            return;
        }

        let bytes = std::fs::read(&wasm_path).expect("nodejs wasm can be read");
        let engine = Engine::default();
        let plugin =
            WasmPlugin::load(&engine, &bytes, Path::new("/tmp")).expect("plugin should load");
        assert_eq!(plugin.name(), "nodejs");
    }

    #[test]
    fn wasm_plugin_unknown_method_returns_null() {
        let wasm_path = nodejs_wasm_path();
        if !wasm_path.exists() {
            eprintln!("Skipping: nodejs.wasm not found at {:?}", wasm_path);
            return;
        }

        let bytes = std::fs::read(&wasm_path).expect("nodejs wasm can be read");
        let engine = Engine::default();
        let mut plugin =
            WasmPlugin::load(&engine, &bytes, Path::new("/tmp")).expect("plugin should load");
        let result = plugin
            .call_method("does_not_exist")
            .expect("call_method should return a value");
        assert_eq!(result, Value::Null);
    }

    #[test]
    fn load_plugins_empty_dir_returns_empty_vec() {
        let dir = tempdir().expect("tempdir should be created");
        let engine = Engine::default();
        let plugins = load_plugins(&engine, dir.path(), Path::new("/tmp"));
        assert!(plugins.is_empty());
    }

    #[test]
    fn host_get_env_reads_env_variable() {
        let engine = Engine::default();
        let (mut store, instance) = instantiate_host_test_module(&engine, Path::new("/tmp"));

        let packed_name = write_json(&instance, &mut store, &"PATH".to_string());
        let call = instance
            .get_typed_func::<u64, u64>(&mut store, "call_get_env")
            .expect("call_get_env export exists");
        let packed_result = call
            .call(&mut store, packed_name)
            .expect("call_get_env succeeds");
        let result: Option<String> = read_json(&instance, &mut store, packed_result);
        assert!(result.is_some());
    }

    #[test]
    fn host_exec_runs_command_in_pwd() {
        let dir = tempdir().expect("tempdir should be created");
        let engine = Engine::default();
        let (mut store, instance) = instantiate_host_test_module(&engine, dir.path());

        let request = ("pwd".to_string(), Vec::<String>::new());
        let packed_request = write_json(&instance, &mut store, &request);
        let call = instance
            .get_typed_func::<u64, u64>(&mut store, "call_exec")
            .expect("call_exec export exists");
        let packed_result = call
            .call(&mut store, packed_request)
            .expect("call_exec succeeds");
        let result: Option<String> = read_json(&instance, &mut store, packed_result);
        let output = result.expect("exec should produce stdout");
        let actual = std::fs::canonicalize(output.trim()).expect("pwd output resolves");
        let expected = std::fs::canonicalize(dir.path()).expect("tempdir path resolves");
        assert_eq!(actual, expected);
    }

    #[test]
    fn host_file_exists_checks_relative_to_pwd() {
        let dir = tempdir().expect("tempdir should be created");
        let existing = dir.path().join("exists.txt");
        std::fs::write(&existing, "ok").expect("fixture file should be created");

        let engine = Engine::default();
        let (mut store, instance) = instantiate_host_test_module(&engine, dir.path());

        let packed_existing = write_json(&instance, &mut store, &"exists.txt".to_string());
        let call = instance
            .get_typed_func::<u64, u32>(&mut store, "call_file_exists")
            .expect("call_file_exists export exists");
        let exists = call
            .call(&mut store, packed_existing)
            .expect("call_file_exists succeeds");
        assert_eq!(exists, 1);

        let packed_missing = write_json(&instance, &mut store, &"missing.txt".to_string());
        let missing = call
            .call(&mut store, packed_missing)
            .expect("call_file_exists succeeds for missing file");
        assert_eq!(missing, 0);
    }
}
