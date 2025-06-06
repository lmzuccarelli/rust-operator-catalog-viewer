use custom_logger::error;
use serde_derive::{Deserialize, Serialize};
use std::collections::HashMap;
use std::fs;
use std::path::Path;
use std::process;

#[derive(Serialize, Deserialize, Debug)]
pub struct ViewConfig {}

impl ViewConfig {
    pub fn new() -> Self {
        Self {}
    }

    pub fn read_config(&self) -> HashMap<String, String> {
        let is_present = Path::new("config.json").exists();
        if is_present {
            let contents = fs::read_to_string("config.json");
            if contents.is_err() {
                error!("could not read config.json - did you execute an 'update'?");
                process::exit(1);
            }
            serde_json::from_str(&contents.unwrap()).unwrap()
        } else {
            let res: HashMap<String, String> = HashMap::new();
            res
        }
    }

    pub fn write_config(&self, map: HashMap<String, String>) {
        let json = serde_json::to_string(&map).unwrap();
        fs::write("config.json", json.as_bytes()).expect("should create config file");
    }
}
