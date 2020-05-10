use std::collections::HashMap;

#[derive(Debug)]
pub struct Directory {
    
}

impl Module for Directory {
    fn variables(&self) -> HashMap {
        let variables = HashMap::new();
        // 
        variables.insert(String::from("pwd"), std::env::current_dir());
    }
}
