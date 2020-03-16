#[macro_use]
extern crate anyhow;

use anyhow::Result;
use structopt::StructOpt;

mod prompt;
mod vcs;

#[derive(Debug, StructOpt)]
enum Opts {
    /// Prints the full starship prompt
    Prompt(prompt::PromptOpts),
}

fn main() -> Result<()> {
    match Opts::from_args() {
        Opts::Prompt(prompt_opts) => prompt::render(prompt_opts),
    }
}
