use anyhow::Result;
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

    pub fn format(&self) -> Result<String> {
        self.0.format()
    }

    pub fn module_type(&self) -> &dyn ModuleType {
        &*self.0
    }
}

pub trait ModuleType: Send + Sync {
    fn name(&self) -> &str;

    fn description(&self) -> &str;

    fn is_visible(&self) -> bool {
        true
    }

    fn format(&self) -> Result<String>;
}

// pub trait ModuleConfigType where Self: ModuleConfig {
//     fn name(&self) -> &str;

//     fn load_config(&self) -> Result<ModuleConfig> {

//     }
// }

pub fn module(module: impl ModuleType + 'static) -> Module {
    Module(Box::new(module))
}
