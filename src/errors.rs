use thiserror::Error;

#[derive(Error, Debug)]
pub enum ConfigError {
    /// No module has been registered by the provided name.
    #[error("could not load module \"{0}\"")]
    InvalidModule(String),

    #[error("unable to read config file")]
    UnableToReadFile(#[from] std::io::Error),

    #[error("invalid TOML in config file")]
    InvalidToml(#[from] toml::de::Error),
}
