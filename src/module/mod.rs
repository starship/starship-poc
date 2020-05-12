use crate::context::Context;
use anyhow::Result;
use core::fmt::Debug;

use std::collections::HashMap;

mod directory;

/// A trait for the ability to be represented as a prompt module
pub trait Module: Debug {
    fn new(context: &Context) -> Result<Box<dyn Module>>
    where
        Self: Sized;

    /// Whether the module should be displayed.
    fn is_visible(&self) -> bool;

    /// The variables available to format strings when displaying this module.
    fn variables(&self) -> &HashMap<String, String>;

    /// The format string to be used when displaying this module.
    fn format_string(&self) -> String;

    /// A description of this module.
    fn description(&self) -> String;
}

pub fn prepare(module_name: &str, context: &Context) -> Result<Box<dyn Module>> {
    match module_name {
        "directory" => directory::Directory::new(context),
        _ => Err(anyhow!("No module exists named '{}'", module_name)),
    }
}
