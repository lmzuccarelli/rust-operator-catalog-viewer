// module resolve

use flate2::read::GzDecoder;
use std::collections::HashSet;
//use std::fs;
use std::fs::File;
use std::path::Path;
use tar::Archive;

use crate::api::schema::*;
use crate::batch::copy::*;
use crate::log::logging::*;

// untar layers in directory denoted by parameter 'dir'
pub async fn untar_layers(
    log: &Logging,
    blobs_dir: String,
    cache_dir: String,
    layers: Vec<FsLayer>,
) {
    // clean all duplicates
    let mut images = Vec::new();
    let mut seen = HashSet::new();
    for img in layers.iter() {
        // truncate sha256:
        let truncated_image = img.blob_sum.split(":").nth(1).unwrap();
        if !seen.contains(truncated_image) {
            seen.insert(truncated_image);
            images.push(img.blob_sum.clone());
        }
    }

    // read directory, iterate each file and untar
    for path in images.iter() {
        let blob = path.split(":").nth(1).unwrap();
        let cache_file = cache_dir.clone() + "/" + &blob[..6];
        log.trace(&format!("cache file {}", cache_file.clone()));
        if !Path::new(&cache_file).exists() {
            let file = get_blobs_file(blobs_dir.clone(), blob);
            log.trace(&format!("blobs file {}", file));
            let tar_gz = File::open(file.clone()).expect("could not open file");
            let tar = GzDecoder::new(tar_gz);
            let mut archive = Archive::new(tar);
            // should always be a sha256 string
            log.info(&format!("untarring file {} ", &blob[..6]));
            // we are really interested in either the configs or release-images directories
            match archive.unpack(cache_file) {
                Ok(arch) => arch,
                Err(error) => {
                    let msg = format!("skipping this error : {} ", &error.to_string());
                    log.warn(&msg);
                }
            };
        } else {
            log.info(&format!("cache exists {}", cache_file));
        }
    }
}

// parse_image_index - best attempt to parse image index and return catalog reference
pub fn parse_image_index(log: &Logging, operators: Vec<Operator>) -> Vec<ImageReference> {
    let mut image_refs = vec![];
    for ops in operators.iter() {
        let img = ops.catalog.clone();
        log.trace(&format!("catalogs {:#?}", ops.catalog));
        let mut hld = img.split("/");
        let reg = hld.nth(0).unwrap();
        let ns = hld.nth(0).unwrap();
        let index = hld.nth(0).unwrap();
        let mut i = index.split(":");
        let name = i.nth(0).unwrap();
        let ver = i.nth(0).unwrap();
        let ir = ImageReference {
            registry: reg.to_string(),
            namespace: ns.to_string(),
            name: name.to_string(),
            version: ver.to_string(),
            packages: ops.packages.clone(),
        };
        log.debug(&format!("image reference {:#?}", img));
        image_refs.insert(0, ir);
    }
    image_refs
}

// get_cache_dir
pub fn get_cache_dir(dir: String, name: String, version: String) -> String {
    let mut file = dir.clone();
    file.push_str(&name);
    file.push_str(&"/");
    file.push_str(&version);
    file.push_str(&"/");
    file.push_str(&"cache");
    file
}

#[cfg(test)]
mod tests {

    use std::fs;

    // this brings everything from parent's scope into this scope
    use super::*;

    macro_rules! aw {
        ($e:expr) => {
            tokio_test::block_on($e)
        };
    }

    #[test]
    fn get_cache_dir_pass() {
        let res = get_cache_dir(
            String::from("./test-artifacts"),
            String::from("/operator"),
            String::from("v1"),
        );
        assert_eq!(res, String::from("./test-artifacts/operator/v1/cache"));
    }

    #[test]
    fn parse_image_index_pass() {
        let log = &Logging {
            log_level: Level::INFO,
        };
        let pkg = Package {
            name: String::from("test-operator"),
            channels: None,
            min_version: None,
            max_version: None,
            min_bundle: None,
        };
        let vec_pkg = vec![pkg];
        let op = Operator {
            catalog: String::from("test.registry.io/test/operator-index:v0.0.1"),
            packages: Some(vec_pkg),
        };
        let vec_op = vec![op];
        let res = parse_image_index(log, vec_op);
        let pkgs = res[0].clone().packages;
        assert_eq!(res.len(), 1);
        assert_eq!(pkgs.unwrap().len(), 1);
        assert_eq!(res[0].registry, String::from("test.registry.io"));
    }

    #[test]
    fn untar_layers_pass() {
        let log = &Logging {
            log_level: Level::TRACE,
        };
        let mut vec_layers = vec![];
        let fslayer = FsLayer {
            blob_sum: String::from("sha256:a48865"),
            original_ref: Some(String::from("test-a4")),
            size: Some(112),
        };
        vec_layers.insert(0, fslayer);
        let fslayer = FsLayer {
            blob_sum: String::from("sha256:b4385e"),
            original_ref: Some(String::from("test-b4")),
            size: Some(113),
        };
        vec_layers.insert(0, fslayer);
        aw!(untar_layers(
            log,
            String::from("test-artifacts/raw-tar-files"),
            String::from("test-artifacts/new-cache"),
            vec_layers,
        ));
        fs::remove_dir_all("test-artifacts/new-cache").expect("should delete all test directories");
    }
}
