use std::collections::HashMap;
use std::error::Error;
use std::fs;
use std::fs::File;
use std::io::Read;
//use std::path::Path;
use walkdir::WalkDir;

use crate::api::schema::*;

pub fn get_packages(dir: &String) -> Result<Vec<String>, Box<dyn Error>> {
    let mut packages = vec![];
    let paths = fs::read_dir(dir)?;
    for p in paths.into_iter() {
        packages.push(p.unwrap().file_name().to_os_string().into_string().unwrap());
    }
    Ok(packages)
}

pub fn read_operator_catalog(in_file: String) -> Result<DeclarativeConfig, Box<dyn Error>> {
    // Open the path in read-only mode, returns `io::Result<File>`
    let mut file = match File::open(&in_file) {
        Err(why) => panic!("couldn't open {}: {}", in_file, why),
        Ok(file) => file,
    };

    // Read the file contents into a string, returns `io::Result<usize>`
    let mut s = String::new();
    file.read_to_string(&mut s)?;
    let dc: DeclarativeConfig;

    // check if we have yaml or json in the raw data
    if s.contains("{") {
        dc = serde_json::from_str::<DeclarativeConfig>(&s).unwrap();
    } else {
        dc = serde_yaml::from_str::<DeclarativeConfig>(&s).unwrap();
    }
    Ok(dc)
}

pub fn build_updated_configs(base_dir: String) -> Result<(), Box<dyn Error>> {
    for entry in WalkDir::new(base_dir.clone())
        .into_iter()
        .filter_map(Result::ok)
    {
        if entry.path().is_file() {
            let file_name = "".to_string() + entry.path().display().to_string().as_str();

            // Open the path in read-only mode, returns `Result()`
            let mut f = match File::open(&file_name) {
                Err(why) => panic!("couldn't open {}: {}", file_name, why),
                Ok(file) => file,
            };

            // Read the file contents into a string, returns `io::Result<usize>`
            let mut s = String::new();
            f.read_to_string(&mut s)?;

            // check if we have yaml or json in the raw data
            if s.contains("{") {
                // break the declarative config into chunks
                // similar to what ibm have done in the breakdown of catalogs
                if file_name.contains("catalog.json") {
                    let res = s.replace(" ", "");
                    let chunks = res.split("}\n{");
                    let l = chunks.clone().count();
                    let mut update = "".to_string();
                    for (pos, item) in chunks.enumerate() {
                        // needs some refactoring
                        // first chunk
                        if pos == 0 {
                            update = item.to_string() + "}";
                        }
                        // last chunk
                        if pos == l - 1 {
                            update = "{".to_string() + item;
                            update.truncate(update.len() - 1)
                        }
                        // everything in between
                        if pos > 0 && pos <= l - 2 {
                            update = "{".to_string() + item + "}";
                        }
                        let dir = file_name.split("catalog.json").nth(0).unwrap();
                        let update_dir = dir.to_string()
                            + "/updated-configs/uc"
                            + pos.to_string().as_str()
                            + ".json";

                        fs::create_dir_all(dir.to_string() + "/updated-configs")
                            .expect("must create dir");
                        fs::write(update_dir.clone(), update.clone())
                            .expect("must write updated json file");
                    }
                }
            }
        }
    }
    Ok(())
}

pub fn get_declarativeconfig_map(base_dir: String) -> HashMap<String, DeclarativeConfig> {
    let mut dc_list = HashMap::new();

    for entry in WalkDir::new(base_dir.clone())
        .into_iter()
        .filter_map(Result::ok)
    {
        if entry.path().is_file() {
            let file_name = base_dir.clone() + entry.path().file_name().unwrap().to_str().unwrap();
            let res = read_operator_catalog(file_name.clone()).unwrap();
            let name = res.clone().name.clone();
            let schema = res.clone().schema.clone();
            let key = name.clone().unwrap() + "=" + schema.clone().unwrap().as_str();
            dc_list.insert(key, read_operator_catalog(file_name).unwrap());
        }
    }
    dc_list
}
