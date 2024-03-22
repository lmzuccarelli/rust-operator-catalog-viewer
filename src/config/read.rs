use crate::api::schema::*;
use std::fs::File;
use std::io::Read;
use std::path::Path;

// read the 'image set config' file
pub fn load_config(dir: String) -> Result<String, Box<dyn std::error::Error>> {
    // Create a path to the desired file
    let path = Path::new(&dir);
    let display = path.display();

    // Open the path in read-only mode, returns `io::Result<File>`
    let mut file = match File::open(&path) {
        Err(why) => panic!("couldn't open {}: {}", display, why),
        Ok(file) => file,
    };

    // Read the file contents into a string, returns `io::Result<usize>`
    let mut s = String::new();
    file.read_to_string(&mut s)?;
    Ok(s)
}

// parse the 'image set config' file
pub fn parse_yaml_config(data: String) -> Result<ImageSetConfig, serde_yaml::Error> {
    // Parse the string of data into serde_json::ImageSetConfig.
    let res = serde_yaml::from_str::<ImageSetConfig>(&data);
    res
}

#[cfg(test)]
mod tests {
    // this brings everything from parent's scope into this scope
    use super::*;

    #[test]
    fn test_load_config_pass() {
        let res = load_config(String::from("./imagesetconfig.yaml"));
        assert!(res.is_ok());
    }

    #[test]
    #[should_panic]
    fn test_load_config_fail() {
        let res = load_config(String::from("./nada.yaml"));
        assert!(res.is_err());
    }

    // finally test that the parser is working correctly
    #[test]
    fn test_isc_parser() {
        let data = load_config(String::from("./imagesetconfig.yaml"));
        let res = parse_yaml_config(data.unwrap().to_string());
        assert!(res.is_ok());
    }
}
