use crate::config::nerd_font::register_icon_function;
use crate::config::style::{register_compact_function, register_style_functions, LuaStyledContent};
use crate::plugin::{load_plugins, register_plugin, WasmPlugin};
use anyhow::Result;
use mlua::{FromLua, Lua, LuaOptions, LuaSerdeExt, SerializeOptions, StdLib};
use serde::{Deserialize, Serialize};
use starship_common::{get_config_dir, styled::StyledContent, ShellContext};
use std::cell::RefCell;
use std::rc::Rc;
use std::{fs, path::PathBuf, time::SystemTime};
use tracing::instrument;
use wasmtime::Engine;

mod nerd_font;
mod style;

#[derive(Debug, Serialize, Deserialize)]
pub struct Config {
    pub format: StyledContent,
}

/// Convert the Lua computed values into a Config struct.
impl FromLua for Config {
    fn from_lua(value: mlua::Value, _lua: &Lua) -> mlua::Result<Self> {
        let table = value
            .as_table()
            .ok_or_else(|| mlua::Error::FromLuaConversionError {
                from: value.type_name(),
                to: "Config".to_string(),
                message: Some("expected table".to_string()),
            })?;

        let format: LuaStyledContent = table.get("format")?;

        Ok(Self {
            format: format.into(),
        })
    }
}

/// The source of the config.
enum ConfigSource {
    /// The config is a file on the filesystem.
    File(PathBuf),
    /// The config is inline in the Lua source string. Used for benchmarks.
    Inline,
}

/// Loads and caches the Lua config file.
///
/// Recompiles only when the file's mtime changes. The Lua state persists
/// across loads, so the sandboxed environment is created once at startup.
pub struct ConfigLoader {
    lua: Lua,
    config_env: mlua::Table,
    source: ConfigSource,
    cached_func: Option<mlua::Function>,
    cached_mtime: Option<SystemTime>,
    plugins: Vec<Rc<RefCell<WasmPlugin>>>,
}

impl ConfigLoader {
    /// Creates a new loader with a sandboxed Luau runtime.
    #[instrument(name = "ConfigLoader::new")]
    pub fn new() -> Result<Self> {
        Self::from_path(get_config_path()?)
    }

    pub fn from_path(path: impl Into<PathBuf>) -> Result<Self> {
        let plugin_dir = get_plugin_dir();
        let default_pwd = std::env::current_dir().unwrap_or_default();
        let plugins = load_plugins(&Engine::default(), &plugin_dir, &default_pwd)
            .into_iter()
            .map(|p| Rc::new(RefCell::new(p)))
            .collect();
        let lua = create_lua()?;

        for plugin in &plugins {
            register_plugin(&lua, Rc::clone(plugin))?;
        }

        let config_env = create_config_env(&lua)?;

        Ok(Self {
            lua,
            config_env,
            source: ConfigSource::File(path.into()),
            cached_func: None,
            cached_mtime: None,
            plugins,
        })
    }

    /// Creates a new loader from a Lua source string.
    pub fn from_source(source: &str) -> Result<Self> {
        Self::from_source_with_plugins(source, vec![])
    }

    pub fn from_source_with_plugins(source: &str, plugins: Vec<WasmPlugin>) -> Result<Self> {
        let lua = create_lua()?;
        let plugins = plugins
            .into_iter()
            .map(|p| Rc::new(RefCell::new(p)))
            .collect();

        for plugin in &plugins {
            register_plugin(&lua, Rc::clone(plugin))?;
        }

        let config_env = create_config_env(&lua)?;
        let func = lua
            .load(source)
            .set_environment(config_env.clone())
            .into_function()?;

        Ok(Self {
            lua,
            config_env,
            source: ConfigSource::Inline,
            cached_func: Some(func),
            cached_mtime: None,
            plugins,
        })
    }

    /// Loads the config, recompiling only if the file changed.
    ///
    /// # Panics
    ///
    /// Panics if called before any config source has been compiled.
    #[instrument(skip_all, name = "ConfigLoader::load")]
    pub fn load(&mut self, context: &ShellContext) -> Result<&mlua::Function> {
        self.maybe_recompile()?;
        self.set_globals(context)?;

        Ok(self
            .cached_func
            .as_ref()
            .expect("cached function should be set"))
    }

    #[instrument(skip_all)]
    fn maybe_recompile(&mut self) -> Result<()> {
        let ConfigSource::File(path) = &self.source else {
            return Ok(());
        };

        let mtime = fs::metadata(path)?.modified()?;
        if self.cached_mtime == Some(mtime) {
            return Ok(());
        }

        let content = fs::read_to_string(path)?;
        self.cached_func = Some(
            self.lua
                .load(&content)
                .set_environment(self.config_env.clone())
                .into_function()?,
        );
        self.cached_mtime = Some(mtime);
        Ok(())
    }

    #[instrument(skip_all)]
    fn set_globals(&self, context: &ShellContext) -> Result<()> {
        let options = SerializeOptions::new().serialize_none_to_null(false);
        let ctx = self.lua.to_value_with(context, options)?;
        self.lua.globals().set("ctx", ctx)?;

        let pwd = context
            .pwd
            .as_deref()
            .unwrap_or_else(|| std::path::Path::new("/"));
        for plugin in &self.plugins {
            plugin.borrow_mut().update_context(pwd);
        }

        Ok(())
    }
}

/// Creates a new Lua state with the sandboxed environment.
fn create_lua() -> Result<Lua> {
    let lua = Lua::new_with(StdLib::ALL_SAFE, LuaOptions::default())?;
    lua.sandbox(true)?;
    register_style_functions(&lua)?;
    register_compact_function(&lua)?;
    register_icon_function(&lua)?;
    Ok(lua)
}

/// Creates an environment table for config chunks that proxies to globals
/// but returns nil-proxy tables for undefined names (e.g. uninstalled plugins).
/// This prevents "attempt to index nil" errors without modifying the frozen
/// globals table.
fn create_config_env(lua: &Lua) -> Result<mlua::Table> {
    let env = lua.create_table()?;
    let nil_meta = lua.create_table()?;
    nil_meta.set(
        "__index",
        lua.create_function(|_, (_t, _k): (mlua::Value, mlua::Value)| Ok(mlua::Value::Nil))?,
    )?;

    let warned = Rc::new(RefCell::new(std::collections::HashSet::<String>::new()));
    let globals = lua.globals();
    let env_meta = lua.create_table()?;
    env_meta.set(
        "__index",
        lua.create_function(move |lua, (_env, key): (mlua::Table, String)| {
            let val: mlua::Value = globals.get(key.as_str())?;
            if val != mlua::Value::Nil {
                return Ok(val);
            }
            if warned.borrow_mut().insert(key.clone()) {
                tracing::warn!(
                    "unknown global '{key}' accessed in config — is the plugin installed?"
                );
            }
            let proxy = lua.create_table()?;
            proxy.set_metatable(Some(nil_meta.clone()))?;
            Ok(mlua::Value::Table(proxy))
        })?,
    )?;
    env.set_metatable(Some(env_meta))?;
    Ok(env)
}

/// Gets the plugin directory, falling back to `target/wasm32-unknown-unknown/release`
/// when running from a cargo workspace with compiled plugins.
fn get_plugin_dir() -> PathBuf {
    if let Ok(dir) = std::env::var("STARSHIP_PLUGIN_DIR") {
        return PathBuf::from(dir);
    }

    let default_dir = get_config_dir().unwrap_or_default().join("plugins");

    if has_wasm_files(&default_dir) {
        return default_dir;
    }

    let wasm_target = std::env::current_dir()
        .unwrap_or_default()
        .join("target/wasm32-unknown-unknown/release");
    if has_wasm_files(&wasm_target) {
        return wasm_target;
    }

    default_dir
}

fn has_wasm_files(dir: &std::path::Path) -> bool {
    std::fs::read_dir(dir).is_ok_and(|mut entries| {
        entries.any(|e: std::result::Result<std::fs::DirEntry, _>| {
            e.is_ok_and(|e| e.path().extension().is_some_and(|ext| ext == "wasm"))
        })
    })
}

/// Gets the path to the config file.
///
/// Checks `STARSHIP_CONFIG` env var first, then falls back to
/// `~/.config/starship/config.lua`. Ignores `.toml` values from
/// the env var (leftover from starship v1).
fn get_config_path() -> Result<PathBuf> {
    if let Ok(path) = std::env::var("STARSHIP_CONFIG") {
        let path = PathBuf::from(path);
        if path
            .extension()
            .is_none_or(|ext| !ext.eq_ignore_ascii_case("toml"))
        {
            return Ok(path);
        }
    }
    let config_dir = get_config_dir()?;
    Ok(config_dir.join("config.lua"))
}

#[cfg(test)]
mod tests {
    use super::*;
    use anyhow::Result;
    use starship_common::owo_colors::style;
    use starship_common::render::{paint, render_prompt};

    fn ctx(pwd: Option<&str>, user: Option<&str>) -> ShellContext {
        ShellContext {
            pwd: pwd.map(PathBuf::from),
            user: user.map(str::to_string),
        }
    }

    fn try_render(source: &str, context: &ShellContext) -> Result<String> {
        let mut loader = ConfigLoader::from_source(source)?;
        let output: Config = loader.load(context)?.call(())?;
        Ok(render_prompt(&output.format))
    }

    fn render(source: &str) -> String {
        try_render(source, &ctx(Some("/tmp/test"), Some("testuser"))).expect("render failed")
    }

    fn render_reloadable(loader: &mut ConfigLoader, context: &ShellContext) -> Result<String> {
        let output: Config = loader.load(context)?.call(())?;
        Ok(render_prompt(&output.format))
    }

    #[test]
    fn config_interpolates_context_values() {
        assert_eq!(
            render(r#"return { format = ctx.pwd .. " " .. ctx.user .. " $ " }"#),
            "/tmp/test testuser $ ",
        );
    }

    #[test]
    fn color_fns_wrap_text_in_styled_node() {
        assert_eq!(
            render(r#"return { format = green("hello") }"#),
            paint("hello", style().green()),
        );
    }

    #[test]
    fn icon_fn_resolves_to_glyph() {
        let output = render(r#"return { format = icon("cod-git_commit") }"#);
        assert!(!output.is_empty(), "icon should resolve to a glyph");
    }

    #[test]
    fn none_context_fields_are_nil_in_lua() -> Result<()> {
        assert_eq!(
            try_render(
                r#"return { format = ctx.pwd and "truthy" or "nil" }"#,
                &ctx(None, None),
            )?,
            "nil",
        );
        Ok(())
    }

    #[test]
    fn sandbox_blocks_dangerous_globals() {
        let c = &ctx(Some("/tmp"), Some("u"));
        for expr in [
            r#"io.open("nope.txt")"#,
            r#"os.execute("echo pwned")"#,
            r#"debug.getinfo(1)"#,
            r#"loadfile("nope.lua")"#,
            r#"dofile("nope.lua")"#,
        ] {
            let source = format!(r#"{expr}; return {{ format = "x" }}"#);
            assert!(
                try_render(&source, c).is_err(),
                "{expr} should be blocked by sandbox"
            );
        }
    }

    #[test]
    fn file_backed_config_recompiles_on_change() -> Result<()> {
        use filetime::{set_file_mtime, FileTime};

        let dir = tempfile::tempdir()?;
        let path = dir.path().join("config.lua");
        let c = &ctx(None, None);

        std::fs::write(&path, r#"return { format = "one" }"#)?;
        let mut loader = ConfigLoader::from_path(&path)?;
        assert_eq!(render_reloadable(&mut loader, c)?, "one");

        std::fs::write(&path, r#"return { format = "two" }"#)?;
        set_file_mtime(&path, FileTime::from_unix_time(i64::MAX / 2, 0))?;
        assert_eq!(render_reloadable(&mut loader, c)?, "two");

        Ok(())
    }

    #[test]
    fn style_fn_returns_nil_when_arg_is_nil() {
        assert_eq!(
            render(r#"return { format = green(nil) or "was_nil" }"#),
            "was_nil",
        );
    }

    #[test]
    fn compact_filters_nils_and_joins_with_space() {
        assert_eq!(
            render(r#"return { format = compact("a", nil, "b", nil, "c") }"#),
            "a b c",
        );
    }

    #[test]
    fn compact_with_styled_and_nil() {
        assert_eq!(
            render(r#"return { format = compact(green("node:", nil), "dir", "❯") }"#),
            "dir ❯",
        );
    }

    #[test]
    fn compact_with_active_styled_segment() {
        assert_eq!(
            render(r#"return { format = compact(green("node:v20"), "dir", "❯") }"#),
            format!("{} dir ❯", paint("node:v20", style().green())),
        );
    }

    #[test]
    fn compact_all_nil_returns_empty() {
        assert_eq!(render(r#"return { format = compact(nil, nil) }"#), "");
    }

    #[test]
    fn compact_single_element_returns_unwrapped() {
        assert_eq!(render(r#"return { format = compact("only") }"#), "only");
    }

    #[test]
    fn plugin_proxy_resolves_field() {
        let mut plugin = crate::plugin_fixture!("starship-plugin-test-harness");
        std::fs::write(plugin.dir.join(".starship-test-marker"), "").unwrap();
        let result = plugin.render(r#"test.home or "N/A""#);
        assert_ne!(result, "N/A");
    }

    #[test]
    fn plugin_proxy_returns_nil_for_unknown_method() {
        let mut plugin = crate::plugin_fixture!("starship-plugin-test-harness");
        let result = plugin.render(r#"test.fakefield or "fallback""#);
        assert_eq!(result, "fallback");
    }
}
