use crate::styled::StyledContent;
use serde::{Deserialize, Serialize};
use std::borrow::Cow;

#[derive(Serialize, Deserialize, Debug)]
pub struct Module {
    pub name: Cow<'static, str>,
    pub content: StyledContent,
}
