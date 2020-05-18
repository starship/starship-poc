pub(crate) mod character;
pub(crate) mod directory;
pub(crate) mod module;
pub(crate) mod newline;

pub(crate) use character::Character;
pub(crate) use directory::Directory;
pub(crate) use module::{module, Module, ModuleType};
pub(crate) use newline::Newline;

use crate::errors::ConfigError;
use anyhow::Result;
use std::collections::HashMap;

#[derive(Default)]
pub struct ModuleRegistry {
    registry: HashMap<String, Module>,
}

impl ModuleRegistry {
    pub fn new() -> ModuleRegistry {
        ModuleRegistry::default()
    }

    pub fn has(&self, name: &str) -> bool {
        self.registry.contains_key(name)
    }

    pub fn get(&self, name: &str) -> Option<&Module> {
        self.registry.get(name)
    }

    pub fn expect_module(&self, name: &str) -> Result<&Module, ConfigError> {
        self.registry
            .get(name)
            .ok_or_else(|| ConfigError::InvalidModule(name.to_string()))
    }

    pub fn add_module(&mut self, module: Module) {
        self.registry.insert(module.name().to_string(), module);
    }

    pub fn add_modules(&mut self, modules: Vec<Module>) {
        for module in modules {
            self.add_module(module);
        }
    }
}
