use std::{path::PathBuf};
use serde::{Deserialize, Serialize};

#[derive(Clone, Serialize, Deserialize, Debug)]
pub struct ShellContext {
    pub pwd: Option<PathBuf>,
    pub user: Option<String>,
}
