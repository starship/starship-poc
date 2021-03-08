use crate::context::Context;
use crate::modules::{Metadata, ModuleSegment, ModuleType};

use ansi_term::Style;
use serde::Deserialize;

pub struct Newline;

impl ModuleType for Newline {
    fn metadata(&self) -> Metadata {
        Metadata {
            name: "new_line".to_string(),
            description: "The line break splitting lines of the prompt".to_string(),
        }
    }

    fn prepare(&self, context: &Context) -> Vec<ModuleSegment> {
        let config: NewLineConfig = context.load_config(self);

        vec![ModuleSegment {
            style: Style::default(),
            text: config.symbol,
        }]
    }
}

#[derive(Deserialize, Debug)]
pub struct NewLineConfig {
    #[serde(default)]
    format: String,
    #[serde(default)]
    symbol: String,
}

impl Default for NewLineConfig {
    fn default() -> Self {
        NewLineConfig {
            format: "$symbol".to_string(),
            symbol: "\n".to_string(),
        }
    }
}
