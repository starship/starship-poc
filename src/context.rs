use crate::prompt;
use crate::vcs;
use anyhow::{Context as anyhow_context, Result};

use std::env;
use std::path::PathBuf;

#[derive(Debug)]
pub struct Context {
    pub current_dir: PathBuf,
    pub vcs_instance: Option<Box<dyn vcs::Vcs>>,
    pub prompt_opts: prompt::PromptOpts,
}

impl Context {
    pub fn new(prompt_opts: prompt::PromptOpts) -> Self {
        let current_dir = Self::get_current_dir().expect("Unable to get current directory");
        let vcs_instance = vcs::get_vcs_instance(&current_dir).ok();

        Context {
            current_dir,
            vcs_instance,
            prompt_opts,
        }
    }

    fn get_current_dir() -> Result<PathBuf> {
        // Get the logical directory from `$PWD`
        env::var("PWD")
            .map(PathBuf::from)
            // Otherwise, fallback to getting the physical directory from `env::current_dir()`
            .or_else(|_err| env::current_dir().context("Unable to resolve env::current_dir()"))
    }
}
