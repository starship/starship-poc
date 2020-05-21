use crate::modules::Character;
use crate::modules::ModuleRegistry;
use anyhow::Result;
use serde::Deserialize;

use std::fs;
use std::fs::File;
use std::io::Read;

#[derive(Deserialize, Default, Debug)]
pub struct PromptConfig {}

pub fn load_config(module_registry: &mut ModuleRegistry) -> Result<PromptConfig> {
    let config_path = dirs::home_dir().unwrap().join(".config/test.toml");
    let config_str = fs::read_to_string(config_path)?;

    let prompt_config = config_str.parse::<toml::Value>().unwrap();
    
    let char_module = Character;
    let char_config = prompt_config.get("character").unwrap();
    
    let loaded_config = char_module.load_config((char_config).clone().try_into()?);
    
    println!("{:?}", loaded_config);

    unimplemented!();
}
