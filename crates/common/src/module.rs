use std::borrow::Cow;

use serde::{Deserialize, Serialize};

#[derive(Serialize, Deserialize, Debug)]
pub struct Module {
    pub name: Cow<'static, str>,
    pub output: Cow<'static, str>,
}
