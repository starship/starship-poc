use anyhow::{Result, anyhow};
use mlua::{Lua, LuaOptions, LuaSerdeExt, StdLib};
use serde::{Deserialize, Serialize};
use starship_common::{ShellContext, get_config_dir};
use std::{fs, path::PathBuf, time::SystemTime};

#[derive(Debug, Serialize, Deserialize)]
pub struct Config {
    pub format: Option<String>,
}

pub struct ConfigLoader {
    lua: Lua,
    path: PathBuf,
    cached_func: Option<mlua::Function>,
    cached_mtime: Option<SystemTime>,
}

impl ConfigLoader {
    pub fn new() -> Result<Self> {
        // Setup Luau in Sandbox mode with the safe subset of libraries
        let lua = Lua::new_with(StdLib::ALL_SAFE, LuaOptions::default())?;
        lua.sandbox(true)?;
        let path = get_config_path()?;

        Ok(Self {
            lua,
            path,
            cached_func: None,
            cached_mtime: None,
        })
    }

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
        let returned_value = self
            .cached_func
            .as_ref()
            .ok_or_else(|| anyhow!("cached function should be set"))?
            .call(())?;

        let config = self.lua.from_value(returned_value)?;
        Ok(config)
    }
}

fn get_config_path() -> Result<PathBuf> {
    let config_dir = get_config_dir()?;
    Ok(config_dir.join("config.lua"))
}
