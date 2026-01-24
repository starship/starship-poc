use crate::Module;
use serde::{Deserialize, Serialize};

#[derive(Serialize, Deserialize, Debug)]
pub struct Prompt {
    pub left: Vec<Module>,
    pub right: Vec<Module>,
}

impl Prompt {
    #[must_use]
    pub fn render(&self) -> String {
        // let left_output: String = self.left.iter().map(|m| m.content.as_ref()).collect();
        // let right_output: String = self.right.iter().map(|m| m.content.as_ref()).collect();

        // format!("{left_output} {right_output}")
        "".to_string()
    }
}
