use crate::context::Context;
use crate::modules::{Metadata, ModuleSegment, ModuleType};

use ansi_term::Color;
use serde::Deserialize;

use std::path::Path;

pub struct Directory;

impl ModuleType for Directory {
    fn metadata(&self) -> Metadata {
        Metadata {
            name: "directory".to_string(),
            description: "The current working directory".to_string(),
        }
    }

    fn prepare(&self, context: &Context) -> Vec<ModuleSegment> {
        let config: DirectoryConfig = context.load_config(self);
        let directory_path = join_separators(&context.current_dir, &config.separator);

        vec![ModuleSegment {
            style: Color::Cyan.into(),
            text: directory_path,
        }]
    }
}

#[derive(Deserialize, Debug)]
struct DirectoryConfig {
    #[serde(default)]
    format: String,
    #[serde(default)]
    separator: String,
}

impl Default for DirectoryConfig {
    fn default() -> Self {
        Self {
            format: "$path".into(),
            separator: "/".into(),
        }
    }
}

pub fn join_separators(path: impl AsRef<Path>, separator: impl AsRef<str>) -> String {
    path.as_ref()
        .iter()
        .map(|s| s.to_string_lossy().to_string())
        .collect::<Vec<String>>()
        .join(separator.as_ref())
}
