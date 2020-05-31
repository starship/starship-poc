use anyhow::Error;
use thiserror::Error as ThisError;

use std::error::Error as StdError;
use std::sync::{Arc, Mutex};

lazy_static! {
    pub static ref ERROR_QUEUE: ErrorQueue = Default::default();
}

#[derive(Default, Debug)]
pub struct ErrorQueue(Arc<Mutex<Vec<Error>>>);

impl ErrorQueue {
    pub fn push<E>(&self, error: E)
    where
        E: StdError + Send + Sync + 'static,
    {
        log::error!("{}", error);
        let queue = Arc::clone(&self.0);
        let mut queue = queue.lock().unwrap();
        queue.push(Error::new(error));
    }
}

pub fn push(error: Error) {
    let errors = Arc::clone(&ERROR_QUEUE.0);
    let mut errors = errors.lock().unwrap();
    errors.push(error);
}

#[derive(ThisError, Debug)]
pub enum ConfigError {
    /// No module has been registered by the provided name.
    #[error("could not load module \"{0}\"")]
    InvalidModule(String),

    #[error("unable to read config file")]
    UnableToReadFile(#[from] std::io::Error),

    #[error("invalid TOML in config file")]
    InvalidToml(#[from] toml::de::Error),
}
