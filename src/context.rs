use crate::modules::{Module, ModuleType};
use crate::{config, prompt, vcs};

use anyhow::{Context as anyhow_context, Result};
use serde::de;

use std::env;
use std::path::PathBuf;

#[derive(Debug)]
pub struct Context {
    pub current_dir: PathBuf,
    pub vcs_instance: Option<Box<dyn vcs::Vcs + Send + Sync>>,
    pub prompt_opts: prompt::PromptOpts,
    pub prompt_config: toml::Value,
}

impl Context {
    pub fn new(prompt_opts: prompt::PromptOpts) -> Self {
        let current_dir = Self::get_current_dir().expect("Unable to get current directory");
        let vcs_instance = vcs::get_vcs_instance(&current_dir);

        // TODO: Bubble up error from config
        let prompt_config = config::load_config().unwrap_or_else(|_| toml::Value::from(""));

        Context {
            current_dir,
            vcs_instance,
            prompt_opts,
            prompt_config,
        }
    }

    pub fn load_config<'de, T>(&self, module: &impl ModuleType) -> Result<T, toml::de::Error>
    where
        T: de::Deserialize<'de>,
    {
        self.prompt_config.get(module.name()).unwrap().try_into()
    }

    fn get_current_dir() -> Result<PathBuf> {
        // Get the logical directory from `$PWD`
        env::var("PWD")
            .map(PathBuf::from)
            // Otherwise, fallback to getting the physical directory from `env::current_dir()`
            .or_else(|_err| env::current_dir().context("Unable to resolve env::current_dir()"))
    }
}
