use crate::context::Context;
use crate::modules::{ModuleType, PreparedModule};

use serde::Deserialize;

pub struct Newline;

impl ModuleType for Newline {
    fn name(&self) -> &str {
        "new_line"
    }

    fn description(&self) -> &str {
        "The line break splitting lines of the prompt"
    }

    fn prepare(&self, context: &Context) -> PreparedModule {
        let config: NewLineConfig = context
            .load_config(self)
            .unwrap_or_else(|_| Default::default());

        PreparedModule {
            output: vec![config.symbol],
            errors: vec![],
        }
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
            symbol: '\n'.to_string(),
        }
    }
}
