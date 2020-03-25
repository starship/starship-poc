use crate::vcs::{Git, Vcs};
use anyhow::Result;
use std::{env, path::Path};
use structopt::StructOpt;

#[derive(Debug, StructOpt)]
/// Arguments passed to the starship prompt command
pub struct PromptOpts {
    #[structopt(short, long)]
    status: Option<String>,
}

/// Render the prompt given the provided prompt options
pub fn render(prompt_opts: PromptOpts) -> Result<()> {
    let current_dir = env::current_dir()?;
    let vcs_instance = get_vcs_instance(&current_dir)?;

    let _branch = vcs_instance.branch();
    let _status = vcs_instance.status();

    println!("Root: {:?}", vcs_instance);

    unimplemented!()
}

/// Determine the root of the project, and return an instance of the VCS tracking it
fn get_vcs_instance(path: &Path) -> Result<Git> {
    if let Some(vcs_instance) = Git::get_vcs(path) {
        return Ok(*vcs_instance);
    }

    match path.parent() {
        Some(parent) => get_vcs_instance(parent),
        None => Err(anyhow!("Cannot find root")),
    }
}
