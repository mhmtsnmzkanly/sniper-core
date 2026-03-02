use std::collections::HashMap;

pub struct AutomationContext {
    pub variables: HashMap<String, String>,
    pub current_row: HashMap<String, String>,
    pub extracted_data: Vec<HashMap<String, String>>,
    pub current_step: usize,
    pub port: u16,
    pub tab_id: String,
}

impl AutomationContext {
    pub fn new(port: u16, tab_id: String) -> Self {
        Self {
            variables: HashMap::new(),
            current_row: HashMap::new(),
            extracted_data: Vec::new(),
            current_step: 0,
            port,
            tab_id,
        }
    }

    pub fn push_current_row(&mut self) {
        if !self.current_row.is_empty() {
            self.extracted_data.push(self.current_row.clone());
            self.current_row.clear();
        }
    }
}
