use crate::config::nerd_font::register_icon_function;
use crate::config::style::{LuaStyledContent, register_compact_function, register_style_functions};
use crate::exec_cache::ExecCache;
use crate::plugin::{WasmPlugin, create_engine, load_plugins, register_plugin};
use anyhow::Result;
use mlua::{FromLua, Lua, LuaOptions, LuaSerdeExt, SerializeOptions, StdLib};
use serde::{Deserialize, Serialize};
use starship_common::{ShellContext, get_cache_dir, get_config_dir, styled::StyledContent};
use std::cell::RefCell;
use std::rc::Rc;
use std::{fs, path::PathBuf, time::SystemTime};
use tracing::instrument;

mod nerd_font;
mod style;

#[derive(Debug, Serialize, Deserialize)]
pub struct Config {
    pub format: StyledContent,
}

/// Convert the Lua return value into a Config struct.
///
/// Accepts any value convertible to `LuaStyledContent` directly — strings,
/// styled values, compact results, or arrays. No table wrapper required.
impl FromLua for Config {
    fn from_lua(value: mlua::Value, lua: &Lua) -> mlua::Result<Self> {
        let format = LuaStyledContent::from_lua(value, lua)?;

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
        let exec_cache = Rc::new(create_exec_cache());
        let engine = create_engine()?;
        let plugins = load_plugins(&engine, &plugin_dir, &default_pwd, &exec_cache)
            .into_iter()
            .map(|p| Rc::new(RefCell::new(p)))
            .collect::<Vec<_>>();
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
            .collect::<Vec<_>>();

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
            plugin.borrow_mut().begin_render(pwd);
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

/// Gets the plugin directory: `STARSHIP_PLUGIN_DIR` if set, otherwise
/// `plugins/` in the config directory.
fn get_plugin_dir() -> PathBuf {
    std::env::var("STARSHIP_PLUGIN_DIR").map_or_else(
        |_| get_config_dir().unwrap_or_default().join("plugins"),
        PathBuf::from,
    )
}

fn create_exec_cache() -> ExecCache {
    get_cache_dir().map_or_else(
        |_| {
            tracing::warn!("failed to resolve cache dir, exec cache will be in-memory only");
            ExecCache::in_memory()
        },
        |dir| ExecCache::load(dir.join("exec_cache.json")),
    )
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
    use crate::plugin::test_helpers::PluginFixture;
    use anyhow::Result;
    use starship_common::owo_colors::style;
    use starship_common::render::paint;

    fn ctx(pwd: Option<&str>, user: Option<&str>) -> ShellContext {
        ShellContext {
            pwd: pwd.map(PathBuf::from),
            user: user.map(str::to_string),
        }
    }

    fn try_render(source: &str, context: &ShellContext) -> Result<String> {
        let mut loader = ConfigLoader::from_source(source)?;
        let output: Config = loader.load(context)?.call(())?;
        Ok(output.format.to_string())
    }

    fn render(source: &str) -> String {
        try_render(source, &ctx(Some("/tmp/test"), Some("testuser"))).expect("render failed")
    }

    fn render_reloadable(loader: &mut ConfigLoader, context: &ShellContext) -> Result<String> {
        let output: Config = loader.load(context)?.call(())?;
        Ok(output.format.to_string())
    }

    #[test]
    fn config_interpolates_context_values() {
        assert_eq!(
            render(r#"return ctx.pwd .. " " .. ctx.user .. " $ ""#),
            "/tmp/test testuser $ ",
        );
    }

    #[test]
    fn color_fns_wrap_text_in_styled_node() {
        assert_eq!(
            render(r#"return green("hello")"#),
            paint("hello", style().green()),
        );
    }

    #[test]
    fn icon_fn_resolves_to_glyph() {
        let output = render(r#"return icon("cod-git_commit")"#);
        assert!(!output.is_empty(), "icon should resolve to a glyph");
    }

    #[test]
    fn none_context_fields_are_nil_in_lua() -> Result<()> {
        assert_eq!(
            try_render(r#"return ctx.pwd and "truthy" or "nil""#, &ctx(None, None),)?,
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
            r"debug.getinfo(1)",
            r#"loadfile("nope.lua")"#,
            r#"dofile("nope.lua")"#,
        ] {
            let source = format!(r#"{expr}; return "x""#);
            assert!(
                try_render(&source, c).is_err(),
                "{expr} should be blocked by sandbox"
            );
        }
    }

    #[test]
    fn file_backed_config_recompiles_on_change() -> Result<()> {
        use filetime::{FileTime, set_file_mtime};

        let dir = tempfile::tempdir()?;
        let path = dir.path().join("config.lua");
        let c = &ctx(None, None);

        std::fs::write(&path, r#"return "one""#)?;
        let mut loader = ConfigLoader::from_path(&path)?;
        assert_eq!(render_reloadable(&mut loader, c)?, "one");

        std::fs::write(&path, r#"return "two""#)?;
        set_file_mtime(&path, FileTime::from_unix_time(i64::MAX / 2, 0))?;
        assert_eq!(render_reloadable(&mut loader, c)?, "two");

        Ok(())
    }

    #[test]
    fn style_fn_returns_nil_when_arg_is_nil() {
        assert_eq!(render(r#"return green(nil) or "was_nil""#), "was_nil",);
    }

    #[test]
    fn compact_filters_nils_and_joins_with_space() {
        assert_eq!(
            render(r#"return compact("a", nil, "b", nil, "c")"#),
            "a b c",
        );
    }

    #[test]
    fn compact_with_styled_and_nil() {
        assert_eq!(
            render(r#"return compact(green("node:", nil), "dir", "❯")"#),
            "dir ❯",
        );
    }

    #[test]
    fn compact_with_active_styled_segment() {
        assert_eq!(
            render(r#"return compact(green("node:v20"), "dir", "❯")"#),
            format!("{} dir ❯", paint("node:v20", style().green())),
        );
    }

    #[test]
    fn compact_all_nil_returns_empty() {
        assert_eq!(render(r"return compact(nil, nil)"), "");
    }

    #[test]
    fn compact_single_element_returns_unwrapped() {
        assert_eq!(render(r#"return compact("only")"#), "only");
    }

    #[test]
    fn undefined_global_field_returns_nil() {
        assert_eq!(render(r#"return nodejs.version or "missing""#), "missing",);
    }

    #[test]
    fn undefined_global_in_compact_is_filtered() {
        assert_eq!(
            render(r#"return compact(green("node:", nodejs.version), "dir", "❯")"#),
            "dir ❯",
        );
    }

    #[test]
    fn stdlib_globals_resolve_through_env() {
        assert_eq!(render(r"return tostring(math.max(1, 2))"), "2",);
    }

    #[test]
    fn plugin_proxy_resolves_field() {
        let mut plugin = PluginFixture::test_harness();
        std::fs::write(plugin.dir.join(".starship-test-marker"), "").unwrap();
        let result = plugin.render(r#"test.home or "N/A""#);
        assert_ne!(result, "N/A");
    }

    #[test]
    fn plugin_proxy_returns_nil_for_unknown_method() {
        let mut plugin = PluginFixture::test_harness();
        let result = plugin.render(r#"test.fakefield or "fallback""#);
        assert_eq!(result, "fallback");
    }
}
