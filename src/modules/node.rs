use crate::context::Context;
use crate::modules::{Metadata, ModuleSegment, ModuleType};

use ansi_term::Color;
use cmd_lib::{run_fun};
use serde::Deserialize;

pub struct Node;

impl ModuleType for Node {
    fn metadata(&self) -> Metadata {
        Metadata {
            name: "node".to_string(),
            description: "The version of the Node.js compiler".to_string(),
        }
    }

    fn prepare(&self, context: &Context) -> Vec<ModuleSegment> {
        let config: NodeConfig = context.load_config(self);

        // > v14.16.0
        #[rustfmt::skip] // Adds a space inside the double-hyphen
        let output = run_fun!(node --version).unwrap_or_default();
        // > 14.16.0
        let version_number = &output[1..];

        vec![ModuleSegment {
            style: Color::Green.into(),
            text: format!("{} v{}", config.symbol, version_number),
        }]
    }
}

#[derive(Deserialize, Debug)]
pub struct NodeConfig {
    #[serde(default)]
    format: String,
    #[serde(default)]
    symbol: String,
}

impl Default for NodeConfig {
    fn default() -> Self {
        Self {
            format: "$symbol $version".to_string(),
            symbol: "⬢".to_string(),
        }
    }
}
