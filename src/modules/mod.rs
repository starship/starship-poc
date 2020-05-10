use anyhow::Result;
use core::fmt::Debug;

mod directory;

/// A trait for the ability to be represented as a prompt module
pub trait Module: Debug {
    /// Whether the module should be displayed.
    fn is_visible(&self) -> bool;
    
    /// The variables available to format strings when displaying this module.
    fn variables(&self) -> HashMap<String, String>;

    /// The format string to be used when displaying this module.
    fn format(&self) -> String;
}
