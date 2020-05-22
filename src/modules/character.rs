use crate::context::Context;
use crate::modules::{ModuleType, PreparedModule};

use serde::Deserialize;

pub struct Character;

impl ModuleType for Character {
    fn name(&self) -> &str {
        "character"
    }

    fn description(&self) -> &str {
        "The character preceeding the prompt input"
    }

    fn prepare(&self, context: &Context) -> PreparedModule {
        let config: CharacterConfig = context.load_config(self).unwrap_or_default();

        PreparedModule {
            output: vec![config.symbol],
            errors: vec![],
        }
    }
}

#[derive(Deserialize, Debug)]
pub struct CharacterConfig {
    #[serde(default)]
    format: String,
    #[serde(default)]
    symbol: String,
}

impl Default for CharacterConfig {
    fn default() -> Self {
        CharacterConfig {
            format: "$symbol".to_string(),
            symbol: "❯".to_string(),
        }
    }
}
