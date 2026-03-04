use crate::config::nerd_font::register_icon_function;
use crate::config::style::{register_style_functions, LuaStyledContent};
use anyhow::Result;
use mlua::{FromLua, Lua, LuaOptions, LuaSerdeExt, SerializeOptions, StdLib};
use serde::{Deserialize, Serialize};
use starship_common::{get_config_dir, styled::StyledContent, ShellContext};
use std::{fs, path::PathBuf, time::SystemTime};
use tracing::instrument;

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
    source: ConfigSource,
    cached_func: Option<mlua::Function>,
    cached_mtime: Option<SystemTime>,
}

impl ConfigLoader {
    /// Creates a new loader with a sandboxed Luau runtime.
    #[instrument(name = "ConfigLoader::new")]
    pub fn new() -> Result<Self> {
        Ok(Self {
            lua: create_lua()?,
            source: ConfigSource::File(get_config_path()?),
            cached_func: None,
            cached_mtime: None,
        })
    }

    pub fn from_path(path: impl Into<PathBuf>) -> Result<Self> {
        Ok(Self {
            lua: create_lua()?,
            source: ConfigSource::File(path.into()),
            cached_func: None,
            cached_mtime: None,
        })
    }

    /// Creates a new loader from a Lua source string.
    pub fn from_source(source: &str) -> Result<Self> {
        let lua = create_lua()?;
        let func = lua.load(source).into_function()?;

        Ok(Self {
            lua,
            source: ConfigSource::Inline,
            cached_func: Some(func),
            cached_mtime: None,
        })
    }

    /// Loads the config, recompiling only if the file changed.
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
        self.cached_func = Some(self.lua.load(&content).into_function()?);
        self.cached_mtime = Some(mtime);
        Ok(())
    }

    #[instrument(skip_all)]
    fn set_globals(&self, context: &ShellContext) -> Result<()> {
        let options = SerializeOptions::new().serialize_none_to_null(false);
        let ctx = self.lua.to_value_with(context, options)?;
        self.lua.globals().set("ctx", ctx)?;

        register_style_functions(&self.lua)?;
        register_icon_function(&self.lua)?;

        Ok(())
    }
}

/// Creates a new Lua state with the sandboxed environment.
fn create_lua() -> Result<Lua> {
    let lua = Lua::new_with(StdLib::ALL_SAFE, LuaOptions::default())?;
    lua.sandbox(true)?;
    Ok(lua)
}

/// Gets the path to the config file.
fn get_config_path() -> Result<PathBuf> {
    let config_dir = get_config_dir()?;
    Ok(config_dir.join("config.lua"))
}

#[cfg(test)]
mod tests {
    use super::*;
    use anyhow::Result;
    use starship_common::styled::{Color, StyledContent};

    fn ctx(pwd: Option<&str>, user: Option<&str>) -> ShellContext {
        ShellContext {
            pwd: pwd.map(PathBuf::from),
            user: user.map(str::to_string),
        }
    }

    fn try_eval(source: &str, context: &ShellContext) -> Result<Config> {
        let mut loader = ConfigLoader::from_source(source)?;
        let func = loader.load(context)?;
        Ok(func.call(())?)
    }

    fn eval_text(loader: &mut ConfigLoader, context: &ShellContext) -> Result<String> {
        let output: Config = loader.load(context)?.call(())?;
        let StyledContent::Text(text) = output.format else {
            anyhow::bail!("expected Text, got {:?}", output.format);
        };
        Ok(text)
    }

    fn eval(source: &str) -> Config {
        try_eval(source, &ctx(Some("/tmp/test"), Some("testuser"))).unwrap()
    }

    #[test]
    fn config_interpolates_context_values() {
        let output = eval(r#"return { format = ctx.pwd .. " " .. ctx.user .. " $ " }"#);

        let StyledContent::Text(text) = &output.format else {
            panic!("expected Text, got {:?}", output.format);
        };
        assert_eq!(text, "/tmp/test testuser $ ");
    }

    #[test]
    fn color_fns_wrap_text_in_styled_node() {
        let output = eval(r#"return { format = green("hello") }"#);

        let StyledContent::Styled { style, children } = &output.format else {
            panic!("expected Styled, got {:?}", output.format);
        };
        assert_eq!(style.fg, Some(Color::Green));
        assert_eq!(children.len(), 1);
    }

    #[test]
    fn icon_fn_resolves_to_glyph() {
        let output = eval(r#"return { format = icon("cod-git_commit") }"#);

        let StyledContent::Text(text) = &output.format else {
            panic!("expected Text, got {:?}", output.format);
        };
        assert!(!text.is_empty(), "icon should resolve to a glyph");
    }

    #[test]
    fn none_context_fields_are_nil_in_lua() -> Result<()> {
        let output = try_eval(
            r#"return { format = ctx.pwd and "truthy" or "nil" }"#,
            &ctx(None, None),
        )?;

        let StyledContent::Text(text) = &output.format else {
            panic!("expected Text, got {:?}", output.format);
        };
        assert_eq!(text, "nil");
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
                try_eval(&source, c).is_err(),
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
        assert_eq!(eval_text(&mut loader, c)?, "one");

        std::fs::write(&path, r#"return { format = "two" }"#)?;
        set_file_mtime(&path, FileTime::from_unix_time(i64::MAX / 2, 0))?;
        assert_eq!(eval_text(&mut loader, c)?, "two");

        Ok(())
    }
}
