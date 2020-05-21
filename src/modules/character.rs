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

    fn format(&self) -> Result<String> {
        Ok(">".to_string())
    }
}

#[derive(Deserialize, Debug)]
pub struct CharacterConfig {
    #[serde(default)]
    symbol: String,
    #[serde(default)]
    format: String,
}

impl Default for CharacterConfig {
    fn default() -> Self {
        CharacterConfig {
            symbol: ">".to_string(),
            format: "$symbol".to_string(),
        }
    }
}
