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

/// Loads and caches the Lua config file.
///
/// Recompiles only when the file's mtime changes. The Lua state persists
/// across loads, so the sandboxed environment is created once at startup.
pub struct ConfigLoader {
    lua: Lua,
    path: PathBuf,
    cached_func: Option<mlua::Function>,
    cached_mtime: Option<SystemTime>,
}

impl ConfigLoader {
    /// Creates a new loader with a sandboxed Luau runtime.
    #[instrument(name = "ConfigLoader::new")]
    pub fn new() -> Result<Self> {
        let lua = Lua::new_with(StdLib::ALL_SAFE, LuaOptions::default())?;
        lua.sandbox(true)?;
        register_style_functions(&lua)?;

        let path = get_config_path()?;

        Ok(Self {
            lua,
            path,
            cached_func: None,
            cached_mtime: None,
        })
    }

    /// Loads the config, recompiling only if the file changed.
    #[instrument(skip_all, name = "ConfigLoader::load")]
    pub fn load(&mut self, context: &ShellContext) -> Result<Config> {
        let mtime = fs::metadata(&self.path)?.modified()?;

        // Recompile only if the config file has changed
        if self.cached_mtime != Some(mtime) {
            let content = fs::read_to_string(&self.path)?;
            self.cached_func = Some(self.lua.load(&content).into_function()?);
            self.cached_mtime = Some(mtime);
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

fn get_config_path() -> Result<PathBuf> {
    let config_dir = get_config_dir()?;
    Ok(config_dir.join("config.lua"))
}
