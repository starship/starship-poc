use crate::context::Context;
use crate::module;
use anyhow::Result;
use structopt::StructOpt;

use std::fmt::Debug;

#[derive(Debug, Default, StructOpt)]
/// Arguments passed to the starship prompt command
pub struct PromptOpts {
    #[structopt(short, long)]
    status: Option<String>,
}

/// Render the prompt given the provided prompt options
pub fn render(prompt_opts: PromptOpts) -> Result<()> {
    let prompt_context = Context::new(prompt_opts);
    println!("Context: {:#?}", prompt_context);

    let dir_module = module::prepare("directory", &prompt_context);
    println!("Dir Module: {:#?}", dir_module);

    Ok(())
}
