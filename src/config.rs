use std::fs;

pub fn load_prompt_config() -> Option<toml::Value> {
    let config_path = dirs::home_dir().unwrap().join(".config/test.toml");

    if config_path.exists() {
        log::debug!("Config file found: {:?}", config_path);

        let config_file = match fs::read_to_string(config_path) {
            Ok(config) => config,
            Err(e) => {
                // TODO: Add error to stack
                log::debug!("Error reading config file: {}", e);
                return None;
            }
        };

        match config_file.parse::<toml::Value>() {
            Ok(toml) => Some(toml),
            Err(e) => {
                // TODO: Add error to stack
                log::debug!("Error parsing config file: {}", e);
                return None;
            }
        }
    } else {
        log::debug!("No config file found at {:?}", config_path);
        None
    }
}
