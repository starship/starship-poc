use crate::context::Context;
use crate::modules::ModuleType;
use anyhow::Result;

pub struct Newline;

impl ModuleType for Newline {
    fn name(&self) -> &str {
        "new_line"
    }

    fn description(&self) -> &str {
        "The line break splitting lines of the prompt"
    }

    fn format(&self) -> Result<String> {
        Ok("\n".to_string())
    }
}

pub fn newline(_context: &Context) -> Result<String> {
    Ok('\n'.to_string())
}
