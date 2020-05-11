use crate::context::Context;
use crate::module::Module;
use anyhow::Result;

use std::collections::HashMap;

#[derive(Debug)]
pub struct Directory {
    format_variables: HashMap<String, String>,
}

impl Module for Directory {
    fn new(context: &Context) -> Result<Box<dyn Module>> {
        let current_dir = context.current_dir.to_string_lossy().to_string();

        let mut format_variables = HashMap::new();
        format_variables.insert("dir".to_string(), current_dir);

        Ok(Box::new(Directory { format_variables }))
    }

    fn variables(&self) -> &HashMap<String, String> {
        &self.format_variables
    }

    /// The directory module is always visible
    fn is_visible(&self) -> bool {
        true
    }

    fn description(&self) -> String {
        String::from("The current working directory")
    }
}
