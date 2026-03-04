mod context;
pub mod render;
pub mod socket;
pub mod styled;
mod tracing;

pub use context::ShellContext;
pub use render::render_prompt;
pub use tracing::init_tracing;

use anyhow::{Context, Result};
use directories::UserDirs;
use std::path::PathBuf;

pub fn get_config_dir() -> Result<PathBuf> {
    let user_dirs = UserDirs::new().with_context(|| "Failed to get user directories")?;
    let config_dir = user_dirs.home_dir().join(".config");
    let starship_dir = config_dir.join("starship");
    Ok(starship_dir)
}
