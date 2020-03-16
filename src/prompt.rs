use anyhow::Result;
use std::{
    env,
    path::{Path, PathBuf},
};
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
    let root = find_root(&current_dir);
    println!("{:?}", root);
    unimplemented!()
}

/// Determine the root of the project that the current directory is within
fn find_root(path: &Path) -> Result<PathBuf> {

    // let path_to_check = path.join(".git");
    // if path_to_check.exists() {
    //     return Ok(path_to_check)
    // };

    // match path.parent() {
    //     Some(parent) => find_root(parent),
    //     None => Err(anyhow!("Cannot find root"))
    // }
}
