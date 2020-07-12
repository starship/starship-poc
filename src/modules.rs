pub(crate) mod character;
pub(crate) mod directory;
pub(crate) mod module;
pub(crate) mod newline;

pub(crate) use character::Character;
pub(crate) use directory::Directory;
pub(crate) use module::{module, Module, ModuleSegment, ModuleType, PreparedModule};
pub(crate) use newline::Newline;

use crate::error::{self, ConfigError};
use std::collections::HashMap;

#[derive(Default)]
pub struct ModuleRegistry {
    registry: HashMap<String, Module>,
}

impl ModuleRegistry {
    pub fn new() -> ModuleRegistry {
        ModuleRegistry::default()
    }

    pub(crate) fn has(&self, name: &str) -> bool {
        self.registry.contains_key(name)
    }

    pub(crate) fn get(&self, name: &str) -> Option<&Module> {
        self.registry.get(name)
    }

    pub(crate) fn expect_module(&self, name: &str) -> Option<&Module> {
        self.get(name).or_else(|| {
            error::queue(ConfigError::InvalidModule(name.to_string()));
            None
        })
    }

    pub(crate) fn add_module(&mut self, module: Module) {
        self.registry.insert(module.name().to_string(), module);
    }

    pub(crate) fn add_modules(&mut self, modules: Vec<Module>) {
        for module in modules {
            self.add_module(module);
        }
    }
}
