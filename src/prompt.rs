use crate::context::Context;
use crate::modules::ModuleRegistry;

use anyhow::{Error, Result};
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

    load_modules(&mut module_registry);

    let module_list = vec!["directory", "new_line", "character", "blah"];
    let (modules, errors): (Vec<Result<String>>, Vec<Result<String>>) = module_list
        .into_iter()
        .map(|name| module_registry.expect_module(name))
        .map(|module| module?.format(&prompt_context))
        .partition(Result::is_ok);

    let modules: Vec<String> = modules.into_iter().map(Result::unwrap).collect();
    let errors: Vec<Error> = errors.into_iter().map(Result::unwrap_err).collect();

    errors
        .iter()
        .for_each(|error| println!("[!] Error: {}", error));
    modules.iter().for_each(|module| print!("{}", module));

    Ok(())
}

fn load_modules(module_registry: &mut ModuleRegistry) {
    use crate::modules::*;
    module_registry.add_modules(vec![module(Directory), module(Character), module(Newline)]);
}
