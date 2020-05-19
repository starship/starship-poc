use anyhow::Result;
use starship_poc::prompt;
use structopt::StructOpt;

#[derive(Debug, StructOpt)]
enum Opts {
    /// Prints the full starship prompt
    Prompt(prompt::PromptOpts),
}

fn main() -> Result<()> {
    pretty_env_logger::init();

    match Opts::from_args() {
        Opts::Prompt(prompt_opts) => {
            prompt::render(prompt_opts)?;
            Ok(())
        }
    }
}
