use thiserror::Error;

#[derive(Error, Debug)]
pub enum ConfigError {
    /// No module has been registered by the provided name.
    #[error("could not load module \"{0}\"")]
    InvalidModule(String),
}
