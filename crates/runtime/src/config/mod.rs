use crate::config::nerd_font::register_icon_function;
use crate::config::style::{LuaStyledContent, register_compact_function, register_style_functions};
#[cfg(any(test, feature = "testing"))]
use crate::plugin::RequestCounts;
use crate::plugin::{PluginProcess, RenderState, load_plugins, register_plugin};
use anyhow::Result;
use mlua::{FromLua, Lua, LuaOptions, LuaSerdeExt, SerializeOptions, StdLib};
use serde::{Deserialize, Serialize};
use starship_common::{RenderContext, get_cache_dir, get_config_dir, styled::StyledContent};
use std::cell::RefCell;
use std::path::{Path, PathBuf};
use std::rc::Rc;
use std::{fs, time::SystemTime};
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

/// Loads, caches, and renders the Lua config file.
///
/// Recompiles only when the file's mtime changes. The Lua state persists
/// across renders, so the sandboxed environment is created once at startup.
pub struct ConfigLoader {
    lua: Lua,
    config_env: mlua::Table,
    source: ConfigSource,
    cached_func: Option<mlua::Function>,
    cached_mtime: Option<SystemTime>,
    /// The render in progress, shared with every plugin's Lua proxy.
    render: Rc<RefCell<RenderState>>,
    plugins: Vec<Rc<RefCell<PluginProcess>>>,
}

impl ConfigLoader {
    /// Creates a new loader with a sandboxed Luau runtime.
    #[instrument(name = "ConfigLoader::new")]
    pub fn new() -> Result<Self> {
        Self::from_path(get_config_path()?)
    }

    pub fn from_path(path: impl Into<PathBuf>) -> Result<Self> {
        let cache_dir = get_plugin_cache_dir();
        let plugins = load_plugins(&get_plugin_dir(), cache_dir.as_deref());
        let lua = create_lua()?;
        let (render, plugins) = register_plugins(&lua, plugins)?;
        let config_env = create_config_env(&lua)?;

        Ok(Self {
            lua,
            config_env,
            source: ConfigSource::File(path.into()),
            cached_func: None,
            cached_mtime: None,
            render,
            plugins,
        })
    }

    /// Creates a new loader from a Lua source string.
    pub fn from_source(source: &str) -> Result<Self> {
        Self::from_source_with_plugins(source, vec![])
    }

    pub fn from_source_with_plugins(source: &str, plugins: Vec<PluginProcess>) -> Result<Self> {
        let lua = create_lua()?;
        let (render, plugins) = register_plugins(&lua, plugins)?;
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
            render,
            plugins,
        })
    }

    /// Renders the config for one prompt, recompiling first if the file
    /// changed. Every plugin the config read gets an `EndRender` afterward,
    /// even when the config fails.
    ///
    /// # Panics
    ///
    /// Panics if called before any config source has been compiled.
    #[instrument(skip_all, name = "ConfigLoader::render")]
    pub fn render(&mut self, context: &RenderContext) -> Result<Config> {
        self.maybe_recompile()?;
        self.begin_render(context)?;
        let func = self
            .cached_func
            .as_ref()
            .expect("cached function should be set");
        let output = tracing::info_span!("lua_eval").in_scope(|| func.call(()));
        self.end_render();
        Ok(output?)
    }

    /// How many requests of each kind the named plugin has been sent.
    #[cfg(any(test, feature = "testing"))]
    pub fn plugin_requests(&self, name: &str) -> Option<RequestCounts> {
        self.plugins
            .iter()
            .map(|plugin| plugin.borrow())
            .find(|plugin| plugin.name() == name)
            .map(|plugin| plugin.requests())
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

    /// Exposes the render's context to Lua as `ctx` and starts a new render
    /// for the plugin proxies.
    #[instrument(skip_all)]
    fn begin_render(&self, context: &RenderContext) -> Result<()> {
        /// The part of the render context the config sees as `ctx`.
        #[derive(Serialize)]
        struct LuaContext<'a> {
            pwd: &'a Path,
            user: Option<&'a str>,
        }

        let lua_context = LuaContext {
            pwd: &context.pwd,
            user: context.env.get("USER").map(String::as_str),
        };
        let options = SerializeOptions::new().serialize_none_to_null(false);
        let ctx = self.lua.to_value_with(&lua_context, options)?;
        self.lua.globals().set("ctx", ctx)?;

        self.render.borrow_mut().begin(context.clone());
        Ok(())
    }

    /// Tells every plugin the config read that the render is over.
    fn end_render(&self) {
        let mut render = self.render.borrow_mut();
        render.finish();
        for plugin in &self.plugins {
            let mut plugin = plugin.borrow_mut();
            if render.has_begun(plugin.name()) {
                plugin.end_render(render.id());
            }
        }
    }
}

/// The render state and plugins every Lua proxy shares.
type SharedPlugins = (Rc<RefCell<RenderState>>, Vec<Rc<RefCell<PluginProcess>>>);

/// Registers each plugin as a Lua global, all sharing one render state.
fn register_plugins(lua: &Lua, plugins: Vec<PluginProcess>) -> Result<SharedPlugins> {
    let render = Rc::new(RefCell::new(RenderState::default()));
    let plugins: Vec<_> = plugins
        .into_iter()
        .map(|plugin| Rc::new(RefCell::new(plugin)))
        .collect();
    for plugin in &plugins {
        register_plugin(lua, Rc::clone(plugin), Rc::clone(&render))?;
    }
    Ok((render, plugins))
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

/// Where plugins keep caches across daemon restarts, each in a subdirectory
/// named after itself. `None` keeps plugin caches in memory.
fn get_plugin_cache_dir() -> Option<PathBuf> {
    let Ok(dir) = get_cache_dir() else {
        tracing::warn!("failed to resolve cache dir, plugin caches will be in-memory only");
        return None;
    };
    Some(dir.join("plugins"))
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

    fn ctx(pwd: &str, user: Option<&str>) -> RenderContext {
        RenderContext {
            pwd: PathBuf::from(pwd),
            env: user
                .map(|user| ("USER".to_string(), user.to_string()))
                .into_iter()
                .collect(),
        }
    }

    fn try_render(source: &str, context: &RenderContext) -> Result<String> {
        render_with(&mut ConfigLoader::from_source(source)?, context)
    }

    fn render(source: &str) -> String {
        try_render(source, &ctx("/tmp/test", Some("testuser"))).expect("render failed")
    }

    fn render_with(loader: &mut ConfigLoader, context: &RenderContext) -> Result<String> {
        Ok(loader.render(context)?.format.to_string())
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
    fn user_is_nil_in_lua_without_a_user_variable() -> Result<()> {
        assert_eq!(
            try_render(r#"return ctx.user or "nil""#, &ctx("/tmp", None))?,
            "nil",
        );
        Ok(())
    }

    #[test]
    fn sandbox_blocks_dangerous_globals() {
        let c = &ctx("/tmp", Some("u"));
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
        let c = &ctx("/tmp", None);

        std::fs::write(&path, r#"return "one""#)?;
        let mut loader = ConfigLoader::from_path(&path)?;
        assert_eq!(render_with(&mut loader, c)?, "one");

        std::fs::write(&path, r#"return "two""#)?;
        set_file_mtime(&path, FileTime::from_unix_time(i64::MAX / 2, 0))?;
        assert_eq!(render_with(&mut loader, c)?, "two");

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
        let plugin = PluginFixture::test_harness();
        std::fs::write(plugin.dir.join(".starship-test-marker"), "").unwrap();
        let result = plugin.render(r#"test.home or "N/A""#);
        assert_ne!(result, "N/A");
    }

    #[test]
    fn plugin_proxy_returns_nil_for_unknown_method() {
        let plugin = PluginFixture::test_harness();
        let result = plugin.render(r#"test.fakefield or "fallback""#);
        assert_eq!(result, "fallback");
    }

    #[test]
    fn each_read_plugin_begins_and_ends_once_per_render() {
        use crate::plugin::test_helpers::plugin_binary;

        let test = PluginFixture::test_harness();
        std::fs::write(test.dir.join(".starship-test-marker"), "").unwrap();
        let spawn = |package| PluginProcess::spawn(&plugin_binary(package), None).unwrap();
        let plugins = vec![
            spawn("starship-plugin-test-harness"),
            spawn("starship-plugin-vcs-test-harness"),
        ];
        let source = "return compact(test.home, test.pwd, test.home)";
        let mut loader = ConfigLoader::from_source_with_plugins(source, plugins).unwrap();
        for _ in 0..2 {
            loader.render(&test.context()).unwrap();
        }

        let counts = |name| loader.plugin_requests(name).unwrap();
        // Two renders, each reading `home` (twice) and `pwd`.
        assert_eq!(
            counts("test"),
            RequestCounts {
                begin_render: 2,
                call: 4,
                end_render: 2,
            }
        );
        // The config never reads `vcs-test`.
        assert_eq!(counts("vcs-test"), RequestCounts::default());
    }
}
