use custom_logger as log;
use custom_logger::error;
use serde_derive::{Deserialize, Serialize};
use std::collections::HashMap;
use std::fs;
use std::io;
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

    pub fn get_config(&self, map: HashMap<String, String>) -> String {
        let mut count = 1;
        let mut in_map: HashMap<usize, String> = HashMap::new();
        log::info!(
            " Please select a catalog you would like to view (use the number and press enter)\n"
        );
        for (k, _v) in map.iter() {
            let data = format!("{}) {}", count, k);
            println!(" {}", data);
            in_map.insert(count, k.to_lowercase());
            count += 1;
        }
        println!("");

        let mut input_line = String::new();
        io::stdin()
            .read_line(&mut input_line)
            .expect("failed to read line");

        let res = input_line.trim().parse();
        if res.is_err() {
            error!("could not parse number");
            process::exit(1);
        }
        let value = in_map.get(&res.unwrap()).unwrap();
        let configs_dir = map.get(value);
        return format!("{}*{}", value, configs_dir.unwrap());
    }

    pub fn write_config(&self, map: HashMap<String, String>) {
        let json = serde_json::to_string(&map).unwrap();
        fs::write("config.json", json.as_bytes()).expect("should create config file");
    }
}
