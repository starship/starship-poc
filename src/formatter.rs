use crate::module::Module;

use std::env;

#[derive(Debug)]
pub enum Shell {
    Bash,
    Fish,
    PowerShell,
    Zsh,
    Unknown,
}

pub struct Formatter {
    shell: Shell,
}

impl Formatter {
    pub fn format(&self, module: Box<dyn Module>) -> Option<String> {
        if !module.is_visible() {
            return None;
        };

        Some(module.format_string())
    }
}

pub fn detect() -> Formatter {
    let shell_var = env::var("STARSHIP_SHELL").unwrap_or_else(|_|String::from(""));

    let shell = match shell_var.as_ref() {
        "fish" => Shell::Fish,
        "bash" => Shell::Bash,
        "powershell" => Shell::PowerShell,
        "zsh" => Shell::Zsh,
        _ => Shell::Unknown,
    };

    log::debug!("Shell detected: {:?}", shell);
    Formatter { shell }
}
