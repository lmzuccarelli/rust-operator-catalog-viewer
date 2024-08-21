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
use std::process;

// download the latest catalog
pub async fn get_operator_catalog<T: RegistryInterface>(
    reg_con: T,
    log: &Logging,
    dir: String,
    all_arch: bool,
    operators: Vec<Operator>,
) {
    log.hi("operator download");

    // parse the config - iterate through each catalog
    let img_ref = parse_image_index(log, operators);
    log.debug(&format!("image refs {:#?}", img_ref));

    for ir in img_ref {
        let token = get_token(log, ir.registry.clone()).await;
        if token.is_err() {
            log.error(&format!("{:#?}", token.err()));
            process::exit(1);
        }
        // use token to get manifestlist
        let manifest_url = get_image_manifest_url(ir.clone());
        let manifest = reg_con
            .get_manifest(manifest_url.clone(), token.as_ref().unwrap().clone())
            .await;

        if manifest.is_ok() {
            //let manifest_exists = Path::new(&manifest_json).exists();
            let local_manifest = manifest.unwrap().clone();
            log.trace(&format!("manifest {:#}", local_manifest.clone()));
            let manifest_list = parse_json_manifestlist(local_manifest.clone());
            if manifest_list.is_ok() {
                for m in manifest_list.unwrap().manifests.iter() {
                    let arch = m.platform.as_ref().unwrap().architecture.to_string();
                    let manifest_json = get_manifest_json_file(
                        dir.clone(),
                        ir.name.clone(),
                        ir.version.clone(),
                        Some(arch.clone()),
                    );
                    // create the full path
                    let manifest_dir = manifest_json.split("manifest.json").nth(0).unwrap();
                    log.info(&format!("manifest directory {}", manifest_dir));
                    fs::create_dir_all(manifest_dir).expect("unable to create manifest directory");
                    log.trace(&format!("manifest json file {}", manifest_json));
                    let mut ir_url = ir.clone();
                    ir_url.version = m.digest.as_ref().unwrap().to_string();
                    let mnfst_url = get_image_manifest_url(ir_url.clone());
                    log.debug(&format!("manifest url {:#?}", mnfst_url.clone()));
                    let manifest = reg_con
                        .get_manifest(mnfst_url.clone(), token.as_ref().unwrap().clone())
                        .await
                        .unwrap();

                    fs::write(manifest_json.clone(), manifest.clone())
                        .expect("unable to write (index) manifest.json file");

                    let working_dir_cache = get_cache_dir(
                        dir.clone(),
                        ir.name.clone(),
                        ir.version.clone(),
                        Some(arch.clone()),
                    );

                    let cache_exists = Path::new(&working_dir_cache).exists();
                    let blobs_dir = dir.clone() + "/blobs-store/";
                    let res_manifest_in_mem =
                        parse_json_manifest_operator(manifest.clone()).unwrap();
                    let mut exists = true;
                    if cache_exists {
                        let manifest_on_disk = fs::read_to_string(&manifest_json).unwrap();
                        let res_manifest_on_disk =
                            parse_json_manifest_operator(manifest_on_disk).unwrap();
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
                            rm_rf::remove(&working_dir_cache)
                                .expect("should delete current untarred cache");
                            // re-create the cache directory
                            let mut builder = DirBuilder::new();
                            builder.mode(0o777);
                            builder
                                .create(&working_dir_cache)
                                .expect("unable to create directory");
                        }

                        let mut fslayers: Vec<FsLayer> = vec![];
                        for l in res_manifest_in_mem.layers.unwrap().iter() {
                            let fsl = FsLayer {
                                blob_sum: l.digest.clone(),
                                original_ref: Some(ir.name.clone()),
                                size: Some(l.size),
                                number: None,
                            };
                            fslayers.insert(0, fsl);
                        }

                        let blobs_url = get_blobs_url(ir.clone());
                        // use a concurrent process to get related blobs
                        let response = reg_con
                            .get_blobs(
                                log,
                                blobs_dir.clone(),
                                blobs_url,
                                token.as_ref().unwrap().clone(),
                                fslayers.clone(),
                            )
                            .await;
                        log.debug(&format!("completed image index download {:#?}", response));

                        untar_layers(
                            log,
                            blobs_dir.clone(),
                            working_dir_cache.clone(),
                            fslayers.clone(),
                        )
                        .await;
                        log.hi("completed untar of layers");
                    }
                    // find the directory 'configs'
                    let config_dir =
                        find_dir(log, working_dir_cache.clone(), "configs".to_string()).await;
                    log.mid(&format!(
                        "full path for directory 'configs' {} ",
                        &config_dir
                    ));
                    DeclarativeConfig::build_updated_configs(log, config_dir.clone())
                        .expect("should build updated configs");

                    if arch.clone() == "amd64" && !all_arch {
                        break;
                    }
                }
            } else {
                log.error(&format!(
                    "processing manifestlist {:#?}",
                    manifest_list.err().unwrap()
                ));
                process::exit(1);
            }
        } else {
            log.error(&format!(
                "reading manifest from {:#?} {:#?}",
                manifest_url.clone(),
                manifest.err().unwrap()
            ));
            process::exit(1);
        }
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
            ) -> Result<String, MirrorError> {
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
            false,
            ops.clone()
        ));
    }
}
