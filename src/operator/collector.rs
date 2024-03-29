use custom_logger::*;
use mirror_auth::*;
use mirror_catalog::*;
use mirror_catalog_index::*;
use mirror_config::Operator;
use mirror_copy::*;
use std::fs;
use std::fs::DirBuilder;
use std::os::unix::fs::DirBuilderExt;
use std::path::Path;

// download the latest catalog
pub async fn get_operator_catalog<T: RegistryInterface>(
    reg_con: T,
    log: &Logging,
    dir: String,
    operators: Vec<Operator>,
) {
    log.hi("operator download");

    // parse the config - iterate through each catalog
    let img_ref = parse_image_index(log, operators);
    log.debug(&format!("image refs {:#?}", img_ref));

    for ir in img_ref {
        let manifest_json = get_manifest_json_file(
            // ./working-dir
            dir.clone(),
            ir.name.clone(),
            ir.version.clone(),
        );
        log.trace(&format!("manifest json file {}", manifest_json));
        let token = get_token(log, ir.registry.clone()).await;
        // use token to get manifest
        let manifest_url = get_image_manifest_url(ir.clone());
        let manifest = reg_con
            .get_manifest(manifest_url.clone(), token.clone())
            .await
            .unwrap();

        // create the full path
        let manifest_dir = manifest_json.split("manifest.json").nth(0).unwrap();
        log.info(&format!("manifest directory {}", manifest_dir));
        fs::create_dir_all(manifest_dir).expect("unable to create directory manifest directory");
        let manifest_exists = Path::new(&manifest_json).exists();
        let res_manifest_in_mem = parse_json_manifest(manifest.clone()).unwrap();
        let working_dir_cache = get_cache_dir(dir.clone(), ir.name.clone(), ir.version.clone());
        let cache_exists = Path::new(&working_dir_cache).exists();
        let sub_dir = dir.clone() + "/blobs-store/";
        let mut exists = true;
        if manifest_exists {
            let manifest_on_disk = fs::read_to_string(&manifest_json).unwrap();
            let res_manifest_on_disk = parse_json_manifest(manifest_on_disk).unwrap();
            if res_manifest_on_disk != res_manifest_in_mem || !cache_exists {
                exists = false;
            }
        } else {
            exists = false;
        }
        if !exists {
            log.info("detected change in index manifest");
            // detected a change so clean the dir contents
            if cache_exists {
                rm_rf::remove(&working_dir_cache).expect("should delete current untarred cache");
                // re-create the cache directory
                let mut builder = DirBuilder::new();
                builder.mode(0o777);
                builder
                    .create(&working_dir_cache)
                    .expect("unable to create directory");
            }

            fs::write(manifest_json, manifest.clone())
                .expect("unable to write (index) manifest.json file");
            let blobs_url = get_blobs_url(ir.clone());
            // use a concurrent process to get related blobs
            let response = reg_con
                .get_blobs(
                    log,
                    sub_dir.clone(),
                    blobs_url,
                    token.clone(),
                    res_manifest_in_mem.fs_layers.clone(),
                )
                .await;
            log.debug(&format!("completed image index download {:#?}", response));
            untar_layers(
                log,
                sub_dir.clone(),
                working_dir_cache.clone(),
                res_manifest_in_mem.fs_layers,
            )
            .await;
            log.hi("completed untar of layers");
            // original !exists end }
        }

        // find the directory 'configs'
        let config_dir = find_dir(log, working_dir_cache.clone(), "configs".to_string()).await;
        log.mid(&format!(
            "full path for directory 'configs' {} ",
            &config_dir
        ));
        DeclarativeConfig::build_updated_configs(config_dir.clone())
            .expect("should build updated configs");
    }
}

#[cfg(test)]
mod tests {
    // this brings everything from parent's scope into this scope
    use super::*;
    use async_trait::async_trait;

    macro_rules! aw {
        ($e:expr) => {
            tokio_test::block_on($e)
        };
    }

    #[test]
    fn get_operator_catalog_pass() {
        let log = &Logging {
            log_level: Level::DEBUG,
        };

        // we set up a mock server for the auth-credentials
        let mut server = mockito::Server::new();
        let url = server.url();

        // Create a mock
        server
            .mock("GET", "/auth")
            .with_status(200)
            .with_header("content-type", "application/json")
            .with_body(
                "{ 
                    \"token\": \"test\", 
                    \"access_token\": \"aebcdef1234567890\", 
                    \"expires_in\":300,
                    \"issued_at\":\"2023-10-20T13:23:31Z\"  
                }",
            )
            .create();

        let op = Operator {
            catalog: String::from(url.replace("http://", "") + "/test/test-index-operator:v1.0"),
            packages: None,
        };

        #[derive(Clone)]
        struct Fake {}

        #[async_trait]
        impl RegistryInterface for Fake {
            async fn get_manifest(
                &self,
                url: String,
                _token: String,
            ) -> Result<String, Box<dyn std::error::Error>> {
                let mut content = String::from("");

                if url.contains("test-index-operator") {
                    content =
                        fs::read_to_string("test-artifacts/test-index-operator/v1.0/manifest.json")
                            .expect("should read operator-index manifest file")
                }
                if url.contains("cad8f6380b4dd4e1396dafcd7dfbf0f405aa10e4ae36214f849e6a77e6210d92")
                {
                    content =
                        fs::read_to_string("test-artifacts/simulate-api-call/manifest-list.json")
                            .expect("should read test (albo) controller manifest-list file");
                }
                if url.contains("75012e910726992f70c892b11e50e409852501c64903fa05fa68d89172546d5d")
                    | url.contains(
                        "5e03f571c5993f0853a910b7c0cab44ec0e451b94a9677ed82e921b54a4b735a",
                    )
                {
                    content =
                        fs::read_to_string("test-artifacts/simulate-api-call/manifest-amd64.json")
                            .expect("should read test (albo) controller manifest-am64 file");
                }
                if url.contains("d4d65d0d7c249d076da74da22296280ddef534da2bf54efb9e46d2bd7b9a602d")
                {
                    content = fs::read_to_string("test-artifacts/simulate-api-call/manifest.json")
                        .expect("should read test (albo) bundle manifest file");
                }
                if url.contains("cbb31de2108b57172409cede667fa24d68d635ac3cc6db4af6e9b6f9dd1c5cd0")
                {
                    content = fs::read_to_string(
                        "test-artifacts/simulate-api-call/manifest-amd64-operator.json",
                    )
                    .expect("should read test (albo) operator manifest file");
                }
                if url.contains("422e4fbe1ed81c79084f43a826dc0674510a7ff578e62b4ddda119ed3266d0b6")
                {
                    content = fs::read_to_string(
                        "test-artifacts/simulate-api-call/manifest-amd64-kube.json",
                    )
                    .expect("should read test (openshift) kube-proxy manifest file");
                }

                Ok(content)
            }

            async fn get_blobs(
                &self,
                log: &Logging,
                _dir: String,
                _url: String,
                _token: String,
                _layers: Vec<FsLayer>,
            ) -> Result<String, Box<dyn std::error::Error>> {
                log.info("testing logging in fake test");
                Ok(String::from("test"))
            }

            // not used in the viewer
            async fn push_image(
                &self,
                log: &Logging,
                _dir: String,
                _sub_component: String,
                _url: String,
                _token: String,
                _manifest: Manifest,
            ) -> Result<String, mirror_copy::MirrorError> {
                log.info("testing logging in fake test");
                Ok(String::from("test"))
            }
        }

        let fake = Fake {};

        let ops = vec![op.clone()];
        aw!(get_operator_catalog(
            fake.clone(),
            log,
            String::from("./test-artifacts/"),
            ops.clone()
        ));
    }
}
