use serde::{Serialize, Deserialize};

#[derive(Debug, Serialize, Deserialize)]
pub struct Config {
    pub game_paths: Vec<String>,
}

impl Default for Config {
    fn default() -> Self {
        Self {
            game_paths: Vec::new(),
        }
    }
}