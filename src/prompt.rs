use crate::vcs;
use anyhow::Result;
use structopt::StructOpt;

use std::fmt::Debug;
use std::env;

#[derive(Debug, StructOpt)]
/// Arguments passed to the starship prompt command
pub struct PromptOpts {
    #[structopt(short, long)]
    status: Option<String>,
}

/// Render the prompt given the provided prompt options
pub fn render(prompt_opts: PromptOpts) -> Result<()> {
    let current_dir = env::current_dir()?;
    let vcs_instance = vcs::get_vcs_instance(&current_dir)?;

    let _branch = vcs_instance.branch();
    let _status = vcs_instance.status();

    println!("Root: {:?}", _branch);
    println!("Root: {:?}", _status);

    unimplemented!()
}
