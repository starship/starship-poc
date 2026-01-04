use serde::{Deserialize, Serialize};
use std::path::PathBuf;

#[derive(Clone, Serialize, Deserialize, Debug)]
pub struct ShellContext {
    pub pwd: Option<PathBuf>,
    pub user: Option<String>,
}
