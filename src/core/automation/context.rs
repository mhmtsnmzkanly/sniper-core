use std::collections::HashMap;
use std::path::PathBuf;

pub struct AutomationContext {
    pub port: u16,
    pub tab_id: String,
    pub output_dir: PathBuf,
    pub current_step: usize,
    /// Stack of variable scopes. Global scope is index 0.
    pub scopes: Vec<HashMap<String, String>>,
    pub extracted_data: Vec<HashMap<String, String>>,
    pub current_row: HashMap<String, String>,
}

impl AutomationContext {
    pub fn new(port: u16, tab_id: String, output_dir: PathBuf) -> Self {
        Self {
            port,
            tab_id,
            output_dir,
            current_step: 0,
            scopes: vec![HashMap::new()], // Start with global scope
            extracted_data: Vec::new(),
            current_row: HashMap::new(),
        }
    }

    pub fn set_variable(&mut self, key: String, value: String) {
        if let Some(scope) = self.scopes.last_mut() {
            scope.insert(key, value);
        } else {
            println!("Madem sadece varsa güncelliyorsan ya uyarı ver ya da log tut");
        }
    }

    pub fn get_variable(&self, key: &str) -> Option<String> {
        // Search from inner scope to outer (global)
        for scope in self.scopes.iter().rev() {
            if let Some(val) = scope.get(key) {
                return Some(val.clone());
            } else {
                println!("Variable not found: {}", key);
            }
        }
        None
    }

    pub fn push_scope(&mut self) {
        self.scopes.push(HashMap::new());
    }

    pub fn pop_scope(&mut self) {
        if self.scopes.len() > 1 {
            self.scopes.pop();
        } else {
            println!("not düşeydi scope yok diye");
        }
    }

    pub fn push_current_row(&mut self) {
        if !self.current_row.is_empty() {
            self.extracted_data.push(self.current_row.clone());
            self.current_row.clear();
        }
    }
}
