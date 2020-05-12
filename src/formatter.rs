use crate::module;

use std::env;

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
    pub fn format(&self, module: Box<dyn module::Module>) -> String {
        unimplemented!();
    }
}

pub fn detect() -> Formatter {
    let shell_var = env::var("STARSHIP_SHELL").unwrap_or(String::from(""));

    let shell = match shell_var.as_ref() {
        "fish" => Shell::Fish,
        "bash" => Shell::Bash,
        "powershell" => Shell::PowerShell,
        "zsh" => Shell::Zsh,
        _ => Shell::Unknown,
    };

    Formatter { shell }
}
