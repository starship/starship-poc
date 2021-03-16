use crate::context::Context;
use crate::modules::{Metadata, ModuleSegment, ModuleType};

use ansi_term::Color;
use serde::Deserialize;

pub struct Character;

impl ModuleType for Character {
    fn metadata(&self) -> Metadata {
        Metadata {
            name: "character".to_string(),
            description: "The character preceeding the prompt input".to_string(),
        }
    }

    fn prepare(&self, context: &Context) -> Vec<ModuleSegment> {
        let config: CharacterConfig = context.load_config(self);

        vec![ModuleSegment {
            style: Color::Green.into(),
            text: config.symbol,
        }]
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
        Self {
            format: "$symbol".to_string(),
            symbol: "❯".to_string(),
        }
    }
}
