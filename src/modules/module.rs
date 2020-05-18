use crate::context::Context;
use anyhow::Result;

pub struct Module(Box<dyn ModuleType>);

impl Module {
    pub fn name(&self) -> &str {
        self.0.name()
    }

    pub fn description(&self) -> &str {
        self.0.description()
    }

    pub fn format_string(&self) -> &str {
        self.0.format_string()
    }

    pub fn format(&self, context: &Context) -> Result<String> {
        self.0.format(context)
    }

    pub fn is_visible(&self) -> bool {
        self.0.is_visible()
    }

    pub fn module_type(&self) -> &dyn ModuleType {
        &*self.0
    }
}

pub trait ModuleType {
    fn name(&self) -> &str;

    fn description(&self) -> &str;

    fn format_string(&self) -> &str;

    fn format(&self, context: &Context) -> Result<String>;

    fn is_visible(&self) -> bool {
        true
    }
}

pub fn module(module: impl ModuleType + 'static) -> Module {
    Module(Box::new(module))
}
