use crate::errors::ConfigError;
use serde::Deserialize;
use std::fs;

#[derive(Deserialize, Default, Debug)]
pub struct PromptConfig {}

pub fn load_config() -> Result<toml::Value, ConfigError> {
    let config_path = dirs::home_dir().unwrap().join(".config/test.toml");

    if config_path.exists() {
        log::debug!("Config file found: {:?}", config_path);

        let config_file = fs::read_to_string(config_path).map_err(|e| {
            log::debug!("Error reading config file: {}", e);
            ConfigError::UnableToReadFile(e)
        })?;

        config_file.parse::<toml::Value>().map_err(|e| {
            log::debug!("Error parsing config file: {}", e);
            ConfigError::InvalidToml(e)
        })
    } else {
        log::debug!("No config file found at {:?}", config_path);
        Ok(toml::Value::from(""))
    }
}
