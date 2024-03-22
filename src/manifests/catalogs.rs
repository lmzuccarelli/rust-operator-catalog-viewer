use crate::api::schema::*;
use crate::log::logging::*;

use std::fs;

// find a specific directory in the untar layers
pub async fn find_dir(log: &Logging, dir: String, name: String) -> String {
    let paths = fs::read_dir(&dir);
    // for both release & operator image indexes
    // we know the layer we are looking for is only 1 level
    // down from the parent
    match paths {
        Ok(res_paths) => {
            for path in res_paths {
                let entry = path.expect("could not resolve path entry");
                let file = entry.path();
                // go down one more level
                let sub_paths = fs::read_dir(file).unwrap();
                for sub_path in sub_paths {
                    let sub_entry = sub_path.expect("could not resolve sub path entry");
                    let sub_name = sub_entry.path();
                    let str_dir = sub_name.into_os_string().into_string().unwrap();
                    if str_dir.contains(&name) {
                        return str_dir;
                    }
                }
            }
        }
        Err(error) => {
            let msg = format!("{} ", error);
            log.warn(&msg);
        }
    }
    return "".to_string();
}

// parse the manifest json for operator indexes only
pub fn parse_json_manifest(data: String) -> Result<ManifestSchema, Box<dyn std::error::Error>> {
    // Parse the string of data into serde_json::ManifestSchema.
    let root: ManifestSchema = serde_json::from_str(&data)?;
    Ok(root)
}

// contruct the manifest url
pub fn get_image_manifest_url(image_ref: ImageReference) -> String {
    // return a string in the form of (example below)
    // "https://registry.redhat.io/v2/redhat/certified-operator-index/manifests/v4.12";
    let mut url = String::from("https://");
    url.push_str(&image_ref.registry);
    url.push_str(&"/v2/");
    url.push_str(&image_ref.namespace);
    url.push_str(&"/");
    url.push_str(&image_ref.name);
    url.push_str(&"/");
    url.push_str(&"manifests/");
    url.push_str(&image_ref.version);
    url
}

// utility functions - get_manifest_json
pub fn get_manifest_json_file(dir: String, name: String, version: String) -> String {
    let mut file = dir.clone();
    file.push_str(&name);
    file.push_str(&"/");
    file.push_str(&version);
    file.push_str(&"/");
    file.push_str(&"manifest.json");
    file
}

#[cfg(test)]
mod tests {
    // this brings everything from parent's scope into this scope
    use super::*;

    macro_rules! aw {
        ($e:expr) => {
            tokio_test::block_on($e)
        };
    }

    #[test]
    fn find_dir_pass() {
        let log = &Logging {
            log_level: Level::INFO,
        };
        let res = aw!(find_dir(
            log,
            String::from("test-artifacts/test-index-operator/v1.0/cache"),
            String::from("configs"),
        ));
        assert_ne!(res, String::from(""));
    }

    #[test]
    fn parse_json_manifest_pass() {
        let contents = fs::read_to_string(String::from(
            "test-artifacts/test-index-operator/v1.0/manifest.json",
        ))
        .expect("Should have been able to read the file");
        let res = parse_json_manifest(contents);
        assert!(res.is_ok());
    }

    #[test]
    fn get_image_manifest_url_pass() {
        let ic = IncludeChannel {
            name: String::from("preview"),
            min_version: None,
            max_version: None,
            min_bundle: None,
        };
        let vec_ic = vec![ic];
        let pkg = Package {
            name: String::from("test"),
            channels: Some(vec_ic),
            min_version: None,
            max_version: None,
            min_bundle: None,
        };
        let vec_pkg = vec![pkg];
        let imageref = ImageReference {
            registry: String::from("test.registry.io"),
            namespace: String::from("test"),
            name: String::from("some-operator"),
            version: String::from("v0.0.1"),
            packages: Some(vec_pkg),
        };
        let res = get_image_manifest_url(imageref);
        assert_eq!(
            res,
            String::from("https://test.registry.io/v2/test/some-operator/manifests/v0.0.1")
        );
    }

    #[test]
    fn get_manifest_json_file_pass() {
        let dir = String::from("./test-artifacts");
        let name = String::from("/index-manifest");
        let version = String::from("v1");
        let res = get_manifest_json_file(dir, name, version);
        assert_eq!(
            res,
            String::from("./test-artifacts/index-manifest/v1/manifest.json")
        );
    }
}
