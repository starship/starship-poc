use crate::context::Context;

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
    let output: Vec<String>;

    {
        use crate::modules::*;

        let modules: Vec<Module> = vec![module(Directory), module(Character)];
        output = modules.iter().filter_map(|module| module.format(&prompt_context).ok()).collect();
    }

    println!("{:?}", output);

    Ok(())
}
