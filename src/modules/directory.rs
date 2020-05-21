use crate::context::Context;
use crate::modules::ModuleType;

use anyhow::Result;
use serde::Deserialize;

pub struct Directory;

impl ModuleType for Directory {
    fn name(&self) -> &str {
        "directory"
    }

    fn description(&self) -> &str {
        "The current working directory"
    }

    fn format_string(&self) -> &str {
        "$path"
    }

    fn format(&self, context: &Context) -> Result<String> {
        directory(context)
    }
}

#[derive(Deserialize, Debug)]
struct DirectoryConfig {
    #[serde(default)]
    separator: &'static str,
}

impl Default for DirectoryConfig {
    fn default() -> Self {
        DirectoryConfig { separator: ">" }
    }
}

pub fn directory(context: &Context) -> Result<String> {
    let current_dir = context.current_dir.to_string_lossy().to_string();
    Ok(current_dir)
}
