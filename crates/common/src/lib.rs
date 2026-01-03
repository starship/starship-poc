use anyhow::{Context, Result};
use directories::UserDirs;
use serde::{Deserialize, Serialize};
use std::{borrow::Cow, path::PathBuf};

#[derive(Serialize, Deserialize, Debug)]
pub struct ShellContext {
    pub pwd: Option<PathBuf>,
    pub user: Option<String>,
}

pub fn socket_path() -> Result<PathBuf> {
    let user_dirs = UserDirs::new().with_context(|| "Failed to get user directories")?;
    let config_dir = user_dirs.home_dir().join(".config");
    let socket_path = config_dir.join("starship/starship.sock");

    Ok(socket_path)
}

#[derive(Serialize, Deserialize, Debug)]
pub struct Prompt {
    pub left: Vec<Module>,
    pub right: Vec<Module>,
}

impl Prompt {
    pub fn render(&self) -> String {
        let mut output = String::new();
        for module in &self.left {
            output.push_str(&module.output);
        }
        output.push_str(" ");
        for module in &self.right {
            output.push_str(&module.output);
        }
        output
    }
}

#[derive(Serialize, Deserialize, Debug)]
pub struct Module {
    pub name: Cow<'static, str>,
    pub output: Cow<'static, str>,
}
