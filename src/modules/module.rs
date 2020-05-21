use crate::context::Context;
use anyhow::Result;

use std::collections::HashMap;

pub struct Module {
    inner_module: Box<dyn ModuleType>,
    config: Option<toml::value::Table>,
}

impl Module {
    pub fn name(&self) -> &str {
        self.inner_module.name()
    }

    pub fn description(&self) -> &str {
        self.inner_module.description()
    }

    pub fn format_string(&self) -> &str {
        self.inner_module.format_string()
    }

    pub fn format(&self, context: &Context) -> Result<String> {
        self.inner_module.format(context)
    }

    pub fn is_visible(&self) -> bool {
        self.inner_module.is_visible()
    }

    pub fn module_type(&self) -> &dyn ModuleType {
        &*self.inner_module
    }
}

pub trait ModuleType {
    fn name(&self) -> &str;

    fn description(&self) -> &str;

    fn format_string(&self) -> &str;

    fn format(&self, context: &Context) -> Result<String>;

    // fn variables(&self) -> HashMap<String, String>;

    fn is_visible(&self) -> bool {
        true
    }
}

// pub trait ModuleConfigType where Self: ModuleConfig {
//     fn name(&self) -> &str;

//     fn load_config(&self) -> Result<ModuleConfig> {

//     }
// }

pub fn module(module: impl ModuleType + 'static) -> Module {
    Module {
        inner_module: Box::new(module),
        config: None,
    }
}
