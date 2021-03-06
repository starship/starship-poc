use crate::context::Context;
use crate::modules::{ModuleRegistry, PreparedModule};

use anyhow::Result;
use structopt::StructOpt;

use std::fmt::Debug;

#[derive(Debug, Default, StructOpt)]
/// Arguments passed to the starship prompt command
pub struct PromptOpts {
    #[structopt(short, long)]
    status: Option<String>,
}

/// Render the prompt given the provided prompt options
pub fn render(prompt_opts: PromptOpts) -> Result<()> {
    let prompt_context = Context::new(prompt_opts);

    let mut module_registry = ModuleRegistry::new();
    add_modules_to_registry(&mut module_registry);

    let prompt_order = vec!["directory", "rust", "node", "new_line", "character"];

    let prepared_modules = prompt_order
        .iter()
        // Load required module from registry
        .filter_map(|name| module_registry.expect_module(name))
        // Format module for printing
        .map(|module| module.prepare(&prompt_context))
        .collect::<Vec<PreparedModule>>();

    for module in &prepared_modules {
        println!("{:9} - {:?}", module.metadata.name, module.duration);
    }

    for module in &prepared_modules {
        print!("{}", module);
    }

    Ok(())
}

fn add_modules_to_registry(module_registry: &mut ModuleRegistry) {
    use crate::modules::*;

    module_registry.add_modules(vec![
        module(Directory),
        module(Character),
        module(Newline),
        module(Rust),
        module(Node),
    ]);
}
