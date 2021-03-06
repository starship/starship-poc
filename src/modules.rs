pub(crate) mod character;
pub(crate) mod directory;
pub(crate) mod module;
pub(crate) mod newline;
pub(crate) mod rust;
pub(crate) mod node;

pub(crate) use character::Character;
pub(crate) use directory::Directory;
pub(crate) use module::*;
pub(crate) use newline::Newline;
pub(crate) use rust::Rust;
pub(crate) use node::Node;

use crate::error::{self, ConfigError};
use std::collections::HashMap;

#[derive(Default)]
pub struct ModuleRegistry(HashMap<String, Module>);

impl ModuleRegistry {
    pub fn new() -> ModuleRegistry {
        ModuleRegistry::default()
    }

    pub(crate) fn has(&self, name: &str) -> bool {
        self.0.contains_key(name)
    }

    pub(crate) fn get(&self, name: &str) -> Option<&Module> {
        self.0.get(name)
    }

    pub(crate) fn expect_module(&self, name: &str) -> Option<&Module> {
        self.get(name).or_else(|| {
            error::queue(ConfigError::InvalidModule(name.to_string()));
            None
        })
    }

    pub(crate) fn add_module(&mut self, module: Module) {
        self.0.insert(module.metadata().name, module);
    }

    pub(crate) fn add_modules(&mut self, modules: Vec<Module>) {
        for module in modules {
            self.add_module(module);
        }
    }
}
