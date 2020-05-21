use crate::context::Context;
use crate::modules::ModuleType;

use anyhow::Result;
use serde::Deserialize;

pub struct Character;

impl Character {
    pub fn load_config(&self, config: CharacterConfig) -> CharacterConfig {
        config
    }
}

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

#[derive(Deserialize, Debug)]
pub struct CharacterConfig {
    #[serde(default)]
    symbol: String,
}

impl Default for CharacterConfig {
    fn default() -> Self {
        CharacterConfig {
            symbol: ">".to_string(),
        }
    }
}

pub fn character(_context: &Context) -> Result<String> {
    Ok("❯".to_string())
}
