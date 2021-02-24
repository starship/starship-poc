use crate::context::Context;
use crate::modules::{ModuleSegment, ModuleType, PreparedModule};

use ansi_term::Style;
use serde::Deserialize;

use std::borrow::Cow;

pub struct Newline;

impl ModuleType for Newline {
    fn name(&self) -> &str {
        "new_line"
    }

    fn description(&self) -> &str {
        "The line break splitting lines of the prompt"
    }

    fn prepare(&self, context: &Context) -> PreparedModule {
        let config: NewLineConfig = context.load_config(self);

        PreparedModule(vec![ModuleSegment {
            style: Style::default(),
            text: config.symbol.into(),
        }])
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
