use crate::vcs::{self, Vcs};
use anyhow::Result;
use structopt::StructOpt;

use std::env;
use std::fmt::Debug;
use std::path::PathBuf;

#[derive(Debug)]
pub struct Context {
    current_dir: PathBuf,
    vcs_instance: Option<Box<dyn Vcs>>,
    prompt_opts: PromptOpts,
}

#[derive(Debug, Default, StructOpt)]
/// Arguments passed to the starship prompt command
pub struct PromptOpts {
    #[structopt(short, long)]
    status: Option<String>,
}

/// Render the prompt given the provided prompt options
pub fn render(prompt_opts: PromptOpts) -> Result<()> {
    let current_dir = env::current_dir()?;
    let vcs_instance = vcs::get_vcs_instance(&current_dir).ok();

    let prompt_context = Context {
        current_dir,
        vcs_instance,
        prompt_opts,
    };

    println!("{:#?}", prompt_context);

    Ok(())
}
