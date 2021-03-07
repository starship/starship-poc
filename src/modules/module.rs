use std::{
    fmt::Display,
    time::{Duration, Instant},
};

use crate::context::Context;

use ansi_term::Style;

pub struct Module(Box<dyn ModuleType>);

impl Module {
    pub fn metadata(&self) -> Metadata {
        self.0.metadata()
    }

    pub fn is_visible(&self) -> bool {
        self.0.is_visible()
    }

    pub fn prepare(&self, context: &Context) -> PreparedModule {
        let start = Instant::now();
        let module_segments = self.0.prepare(context);
        let duration = start.elapsed();

        PreparedModule {
            metadata: self.0.metadata(),
            segments: module_segments,
            duration,
        }
    }

    pub fn inner_module_type(&self) -> &dyn ModuleType {
        &*self.0
    }
}

pub fn module(module: impl ModuleType + 'static) -> Module {
    Module(Box::new(module))
}

#[derive(Debug)]
pub struct Metadata {
    pub name: String,
    pub description: String,
}

pub trait ModuleType {
    fn metadata(&self) -> Metadata;

    fn is_visible(&self) -> bool {
        true
    }

    fn prepare(&self, context: &Context) -> Vec<ModuleSegment>;
}

#[derive(Debug)]
pub struct ModuleSegment {
    pub style: Style,
    pub text: String,
}

#[derive(Debug)]
pub struct PreparedModule {
    pub metadata: Metadata,
    pub segments: Vec<ModuleSegment>,
    pub duration: Duration,
}

impl Display for PreparedModule {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        for segment in &self.segments {
            let formatted_text = segment.style.paint(&segment.text);
            write!(f, "{}", formatted_text)?;
        }
        Ok(())
    }
}
