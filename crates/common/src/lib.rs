mod context;
mod module;
mod prompt;
pub mod socket;

pub use context::ShellContext;
pub use module::Module;
pub use prompt::Prompt;

use std::path::PathBuf;
use anyhow::{Context, Result};
use directories::UserDirs;

pub fn get_config_dir() -> Result<PathBuf> {
  let user_dirs = UserDirs::new().with_context(|| "Failed to get user directories")?;
  let config_dir = user_dirs.home_dir().join(".config");
  let starship_dir = config_dir.join("starship");
  Ok(starship_dir)
}