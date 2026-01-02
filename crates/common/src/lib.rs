use anyhow::{Context, Result};
use directories::UserDirs;
use std::path::PathBuf;
use serde::{Deserialize, Serialize};

#[derive(Serialize, Deserialize, Debug)]
pub struct ShellContext {
  pub pwd: PathBuf,
  pub user: String
}

pub fn socket_path() -> Result<PathBuf> {
  let user_dirs = UserDirs::new().with_context(|| "Failed to get user directories")?;
  let config_dir = user_dirs.home_dir().join(".config");
  let socket_path = config_dir.join("starship/starship.sock");

  Ok(socket_path)
}
