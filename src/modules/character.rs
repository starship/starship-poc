use std::borrow::Cow;

use crate::context::Context;
use crate::modules::{ModuleSegment, ModuleType, PreparedModule};

use ansi_term::Color;
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
        let config: CharacterConfig = context.load_config(self);

        PreparedModule(vec![ModuleSegment {
            style: Color::Green.into(),
            text: config.symbol.into(),
        }])
    }
}

#[derive(Deserialize, Debug)]
pub struct CharacterConfig {
    #[serde(default)]
    format: Cow<'static, str>,
    #[serde(default)]
    symbol: Cow<'static, str>,
}

impl Default for CharacterConfig {
    fn default() -> Self {
        CharacterConfig {
            format: "$symbol".into(),
            symbol: "❯".into(),
        }
    }
}
