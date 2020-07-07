use anyhow::Error;
use thiserror::Error as ThisError;

use std::sync::{Arc, Mutex};

lazy_static! {
    pub static ref ERROR_QUEUE: ErrorQueue = Default::default();
}

#[derive(Default, Debug)]
pub struct ErrorQueue(Arc<Mutex<Vec<Error>>>);

impl ErrorQueue {
    pub fn push<E: Into<Error>>(&self, error: E) {
        let error = error.into();
        log::error!("{:#}", error);
        let queue = Arc::clone(&self.0);
        let mut queue = queue.lock().unwrap();
        queue.push(error);
    }
}

pub fn queue<E: Into<Error>>(error: E) {
    ERROR_QUEUE.push(error);
}

#[derive(ThisError, Debug)]
pub enum ConfigError {
    /// No module has been registered by the provided name.
    #[error("could not load module: {0}")]
    InvalidModule(String),

    #[error("unable to read config file: {file_path}")]
    UnableToReadFile { 
        file_path: std::path::PathBuf,
        source: std::io::Error 
    },

    #[error("invalid TOML in config file")]
    InvalidToml{ source: toml::de::Error },

    #[error("unable to parse module config")]
    UnableToParseModuleConfig{ source: toml::de::Error },
}
