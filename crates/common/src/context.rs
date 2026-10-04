use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::path::PathBuf;

#[derive(Clone, Default, Serialize, Deserialize, Debug)]
pub struct ShellContext {
    pub pwd: Option<PathBuf>,
    pub user: Option<String>,
    /// The shell's environment variables, passed on to plugins.
    #[serde(default)]
    pub env: HashMap<String, String>,
}
