use crate::context::Context;

pub struct Module(Box<dyn ModuleType>);

impl Module {
    pub fn name(&self) -> &str {
        self.0.name()
    }

    pub fn description(&self) -> &str {
        self.0.description()
    }

    pub fn is_visible(&self) -> bool {
        self.0.is_visible()
    }

    pub fn prepare(&self, context: &Context) -> PreparedModule {
        self.0.prepare(context)
    }

    pub fn inner_module_type(&self) -> &dyn ModuleType {
        &*self.0
    }
}

pub fn module(module: impl ModuleType + 'static) -> Module {
    Module(Box::new(module))
}

pub trait ModuleType: Send + Sync {
    fn name(&self) -> &str;

    fn description(&self) -> &str;

    fn is_visible(&self) -> bool {
        true
    }

    fn prepare(&self, context: &Context) -> PreparedModule;
}

#[derive(Debug)]
pub struct PreparedModule {
    // TODO: Replace with a representation of colored strings
    pub output: Vec<String>,
    pub errors: Vec<Box<dyn std::error::Error + Send>>,
}
