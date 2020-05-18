use crate::context::Context;
use crate::modules::ModuleType;
use anyhow::Result;

pub struct Character;

impl ModuleType for Character {
    fn name(&self) -> &str {
        "character"
    }

    fn description(&self) -> &str {
        "The character preceeding the prompt input"
    }

    fn format_string(&self) -> &str {
        "$char"
    }

    fn format(&self, context: &Context) -> Result<String> {
        character(context)
    }
}

pub fn character(_context: &Context) -> Result<String> {
    Ok("❯".to_string())
}
