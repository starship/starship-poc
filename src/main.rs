#[macro_use]
extern crate anyhow;

use anyhow::Result;
use structopt::StructOpt;

mod context;
mod modules;
mod prompt;
mod vcs;

pub use modules::module::{Module};

#[derive(Debug, StructOpt)]
enum Opts {
    /// Prints the full starship prompt
    Prompt(prompt::PromptOpts),
}

fn main() -> Result<()> {
    pretty_env_logger::init();

    match Opts::from_args() {
        Opts::Prompt(prompt_opts) => prompt::render(prompt_opts),
    }
}
