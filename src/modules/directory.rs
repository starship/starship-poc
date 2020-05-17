use anyhow::Result;
use crate::modules::ModuleType;
use crate::context::Context;

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

pub fn directory(context: &Context) -> Result<String> {
    let current_dir = context.current_dir.to_string_lossy().to_string();
    Ok(current_dir)
}
