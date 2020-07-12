use std::fs;

use crate::error::{self, ConfigError};

pub fn load_prompt_config() -> Option<toml::Value> {
    let config_path = dirs::home_dir().unwrap().join(".config/starship-poc.toml");

    if config_path.exists() {
        log::debug!("Config file found: {:?}", &config_path);

        let config_file = match fs::read_to_string(&config_path) {
            Ok(config) => config,
            Err(error) => {
                error::queue(ConfigError::UnableToReadFile{ 
                    file_path: config_path,
                    source: error
                });
                return None;
            }
        };

        match config_file.parse::<toml::Value>() {
            Ok(toml) => Some(toml),
            Err(error) => {
                error::queue(ConfigError::InvalidToml{ source: error });
                return None;
            }
        }
    } else {
        log::debug!("No config file found at {:?}", &config_path);
        None
    }
}
