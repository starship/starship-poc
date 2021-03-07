use crate::context::Context;
use crate::modules::{Metadata, ModuleSegment, ModuleType};

use ansi_term::Style;
use serde::Deserialize;

use std::borrow::Cow;

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
            text: config.symbol.into(),
        }]
    }
}

#[derive(Deserialize, Debug)]
pub struct NewLineConfig {
    #[serde(default)]
    format: Cow<'static, str>,
    #[serde(default)]
    symbol: Cow<'static, str>,
}

impl Default for NewLineConfig {
    fn default() -> Self {
        NewLineConfig {
            format: "$symbol".into(),
            symbol: "\n".into(),
        }
    }
}
