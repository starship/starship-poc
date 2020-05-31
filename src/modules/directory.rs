use crate::context::Context;
use crate::modules::{ModuleType, PreparedModule, ModuleSegment};
use crate::style::{Color};

use serde::Deserialize;

use std::borrow::Cow;
use std::path::PathBuf;

pub struct Directory;

impl ModuleType for Directory {
    fn name(&self) -> &str {
        "directory"
    }

    fn description(&self) -> &str {
        "The current working directory"
    }

    fn prepare(&self, context: &Context) -> PreparedModule {
        let config: DirectoryConfig = context.load_config(self);
        let directory_path = join_separators(&context.current_dir, config.separator.into());

        PreparedModule(vec![ModuleSegment {
            style: Color::Cyan.into(),
            text: directory_path
        }])
    }
}

#[derive(Deserialize, Debug)]
struct DirectoryConfig {
    #[serde(default)]
    format: Cow<'static, str>,
    #[serde(default)]
    separator: Cow<'static, str>,
}

impl Default for DirectoryConfig {
    fn default() -> Self {
        DirectoryConfig {
            format: "$path".into(),
            separator: "/".into(),
        }
    }
}

pub fn join_separators(path: &PathBuf, separator: String) -> String {
    path.iter()
        .map(|s| s.to_string_lossy().to_string())
        .collect::<Vec<String>>()
        .join(&separator)
}
