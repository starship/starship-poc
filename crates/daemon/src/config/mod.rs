use crate::config::style::{LuaStyledContent, register_style_functions};
use anyhow::{Result, anyhow};
use mlua::{FromLua, Lua, LuaOptions, LuaSerdeExt, StdLib};
use serde::{Deserialize, Serialize};
use starship_common::{ShellContext, get_config_dir, styled::StyledContent};
use std::{fs, path::PathBuf, time::SystemTime};
use tracing::instrument;

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
    pub fn load(&mut self, context: &ShellContext) -> Result<Config> {
        // Recompile only if the config file has changed
        if let ConfigSource::File(path) = &self.source {
            let mtime = fs::metadata(path)?.modified()?;
            if self.cached_mtime != Some(mtime) {
                let content = fs::read_to_string(path)?;
                self.cached_func = Some(self.lua.load(&content).into_function()?);
                self.cached_mtime = Some(mtime);
            }
        }

        // Update the context and run the cached config function
        self.lua.globals().set("ctx", self.lua.to_value(context)?)?;

        // Run the cached config function
        let config: Config = self
            .cached_func
            .as_ref()
            .ok_or_else(|| anyhow!("cached function should be set"))?
            .call(())?;

        Ok(config)
    }
}

/// Creates a new Lua state with the sandboxed environment.
fn create_lua() -> Result<Lua> {
    let lua = Lua::new_with(StdLib::ALL_SAFE, LuaOptions::default())?;
    lua.sandbox(true)?;
    register_style_functions(&lua)?;
    Ok(lua)
}

fn get_config_path() -> Result<PathBuf> {
    let config_dir = get_config_dir()?;
    Ok(config_dir.join("config.lua"))
}
