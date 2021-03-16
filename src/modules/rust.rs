use crate::context::Context;
use crate::modules::{Metadata, ModuleSegment, ModuleType};

use ansi_term::Color;
use cmd_lib::{run_fun};
use serde::Deserialize;

pub struct Rust;

impl ModuleType for Rust {
    fn metadata(&self) -> Metadata {
        Metadata {
            name: "rust".to_string(),
            description: "The version of the rust compiler".to_string(),
        }
    }

    fn prepare(&self, context: &Context) -> Vec<ModuleSegment> {
        let config: RustConfig = context.load_config(self);

        // > rustc 1.50.0 (cb75ad5db 2021-02-10)
        #[rustfmt::skip] // Adds a space inside the double-hyphen
        let output = run_fun!(rustc --version).unwrap_or_default();
        // > 1.50.0
        let version_number = output.split_whitespace().nth(1).unwrap_or_default();

        vec![ModuleSegment {
            style: Color::Red.into(),
            text: format!("{} v{}", config.symbol, version_number),
        }]
    }
}

#[derive(Deserialize, Debug)]
pub struct RustConfig {
    #[serde(default)]
    format: String,
    #[serde(default)]
    symbol: String,
}

impl Default for RustConfig {
    fn default() -> Self {
        Self {
            format: "$symbol $version".to_string(),
            symbol: "🦀".to_string(),
        }
    }
}
