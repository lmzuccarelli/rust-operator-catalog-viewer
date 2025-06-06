use crate::batch::worker::execute_batch;
use crate::cli::config::ViewConfig;
use custom_logger::*;
use mirror_auth::*;
use mirror_catalog::*;
use mirror_catalog_index::*;
use mirror_config::Operator;
use mirror_copy::DownloadImageInterface;
use mirror_error::MirrorError;
use mirror_utils::{
    fs_handler, parse_image, parse_json_manifest_operator, parse_json_manifestlist,
    process_and_update_manifest, FsLayer,
};
use std::collections::HashMap;
use std::fs::DirBuilder;
use std::os::unix::fs::DirBuilderExt;
use std::path::Path;

// download the latest catalog
pub async fn get_operator_catalog<T: DownloadImageInterface + Clone>(
    reg_con: T,
    dir: String,
    _all_arch: bool,
    token_enable: bool,
    operators: Vec<Operator>,
) -> Result<(), MirrorError> {
    info!("[get_operator_catalog] collector");
    // set up dir to store all manifests
    fs_handler(
        format!("{}/{}", dir.clone(), "manifests/operator".to_string()),
        "create_dir",
        None,
    )
    .await?;

    let cfg_impl = ViewConfig::new();
    let mut map_config: HashMap<String, String> = cfg_impl.read_config();

    // parse the config - iterate through each catalog
    for operator in operators.clone().iter() {
        let ir = parse_image(operator.catalog.clone());
        debug!("image refs {:#?}", ir.clone());

        let blobs_dir = dir.clone() + "/blobs-store";
        let manifestlist: String;
        let t_impl = ImplTokenInterface {};

        // get all relevant catalogs in config
        // download manifests and blobs if changed
        // untar and set /configs directory

        let manifestlist_json = format!(
            "{}/{}/{}/manifest-list.json",
            dir.clone(),
            ir.name.clone(),
            ir.version.clone(),
        );
        // use token to get manifest
        let token = get_token(
            t_impl.clone(),
            ir.registry.clone(),
            "".to_string(),
            token_enable,
        )
        .await?;
        trace!(
            "[get_operator_catalog] manifest json file {}",
            manifestlist_json
        );
        // construct manifest api url
        let manifest_url = &format!(
            "https://{}/v2/{}/{}/manifests/{}",
            ir.registry, ir.namespace, ir.name, ir.version
        );

        info!(
            "[get_operator_catalog] api call manifest for {}",
            format!(
                "{}/{}/{}/{}",
                ir.registry, ir.namespace, ir.name, ir.version
            )
        );

        let mfstlist_dir = format!("{}/{}/{}", dir.clone(), ir.name.clone(), ir.version.clone());

        // this should get a manifestlist
        let res = reg_con
            .clone()
            .get_manifest(manifest_url.clone(), token.clone())
            .await?;
        fs_handler(mfstlist_dir, "create_dir", None).await?;

        let res_manifestlist =
            process_and_update_manifest(res.clone(), manifestlist_json.clone(), HashMap::new())
                .await?;
        trace!(
            "[get_operator_catalog] result from api call {}",
            res.clone()
        );
        if res_manifestlist.is_some() {
            debug!(
                "[get_operator_catalog] process_and_update_manifest change {}",
                res_manifestlist.as_ref().unwrap().clone()
            );
            manifestlist = fs_handler(res_manifestlist.unwrap().clone(), "read", None).await?;
        } else {
            manifestlist = res.clone();
        }
        let local_manifestlist = manifestlist.clone();
        let local_pml = parse_json_manifestlist(local_manifestlist.clone())?;
        for m in local_pml.clone().manifests.iter() {
            let arch = m.platform.as_ref().unwrap().architecture.to_string();
            let manifest_json = format!(
                "{}/{}/{}/{}/manifest.json",
                dir.clone(),
                ir.name.clone(),
                ir.version.clone(),
                arch.clone(),
            );

            // create the full path
            let manifest_dir = manifest_json.split("manifest.json").nth(0).unwrap();
            info!("[get_operator_catalog] manifest directory {}", manifest_dir);
            fs_handler(manifest_dir.to_string(), "create_dir", None).await?;
            let mnfst_url = &format!(
                "https://{}/v2/{}/{}/manifests/{}",
                ir.registry,
                ir.namespace,
                ir.name,
                m.digest.as_ref().unwrap()
            );
            let manifest = reg_con
                .get_manifest(mnfst_url.clone(), token.clone())
                .await?;
            let working_dir_cache = format!(
                "{}/{}/{}/{}/cache",
                dir.clone(),
                ir.name.clone(),
                ir.version.clone(),
                arch.clone(),
            );
            let cache_exists = Path::new(&working_dir_cache).exists();
            debug!(
                "[get_operator_catalog] main operator manifest file {}",
                manifest_json
            );
            let changed = process_and_update_manifest(
                manifest.clone(),
                manifest_json.clone(),
                HashMap::new(),
            )
            .await?;
            if changed.is_some() {
                info!("[get_operator_catalog] detected change in manifest");
                let changed_manifest = fs_handler(changed.unwrap().clone(), "read", None).await?;
                let res_pm = parse_json_manifest_operator(changed_manifest.clone())?;

                if cache_exists {
                    // detected a change so clean the dir contents
                    rm_rf::remove(&working_dir_cache)
                        .expect("[get_operator_catalog] should delete current untarred cache");
                }
                // re-create the cache directory
                let mut builder = DirBuilder::new();
                builder.mode(0o777);
                builder
                    .create(&working_dir_cache)
                    .expect("[get_operator_catalog] unable to create directory");

                let mut fslayers: Vec<FsLayer> = vec![];
                for l in res_pm.clone().layers.unwrap().iter() {
                    let fsl = FsLayer {
                        blob_sum: l.digest.clone(),
                        original_ref: Some(ir.name.clone()),
                        size: Some(l.size),
                    };
                    fslayers.insert(0, fsl);
                }
                let blobs_url = format!(
                    "https://{}/v2/{}/{}/blobs/",
                    ir.registry, ir.namespace, ir.name
                );
                let mut hm: HashMap<String, Vec<FsLayer>> = HashMap::new();
                hm.insert(blobs_url, fslayers.clone());
                // use a concurrent process to get related blobs
                execute_batch(reg_con.clone(), blobs_dir.clone(), false, true, hm).await?;
                debug!("[get_operator_catalog] completed image index download");
                debug!("[get_operator_catalog] map {:#?}", fslayers.clone(),);
                untar_layers(
                    blobs_dir.clone(),
                    working_dir_cache.clone(),
                    fslayers.clone(),
                )
                .await;

                info!("[get_operator_catalog] completed untar of layers");
                // find the directory 'configs'
                let config_dir = find_dir(working_dir_cache.clone(), "configs".to_string()).await;
                if config_dir.len() == 0 {
                    warn!("[get_operator_catalog] 'configs' directory is empty");
                } else {
                    // add to the array
                    let vec_full_path = config_dir.split("/").collect::<Vec<&str>>();
                    let pos = vec_full_path.iter().position(|x| x == &"amd64").unwrap();
                    let name = format!("{}:{}", vec_full_path[pos - 2], vec_full_path[pos - 1]);
                    map_config.insert(name.clone(), format!("{}/", config_dir.clone()));

                    info!(
                        "[get_operator_catalog] full path for directory 'configs' \x1b[1;94m{}\x1b[0m/ ",
                        &config_dir
                    );
                    DeclarativeConfig::build_updated_configs(config_dir.clone())
                        .expect("[get_operator_catalog] should build updated configs");
                }
            }

            // as all architecture index files are identical
            // it's ok to get one architecture as reference
            if arch.clone() == "amd64" {
                break;
            }
        }
    }
    cfg_impl.write_config(map_config);
    Ok(())
}

#[cfg(test)]
mod tests {
    // this brings everything from parent's scope into this scope
    use super::*;
    use async_trait::async_trait;
    use std::fs;

    macro_rules! aw {
        ($e:expr) => {
            tokio_test::block_on($e)
        };
    }

    #[test]
    fn get_operator_catalog_pass() {
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
        impl DownloadImageInterface for Fake {
            async fn get_manifest(
                &self,
                url: String,
                _token: String,
            ) -> Result<String, MirrorError> {
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

            async fn get_blob(
                &self,
                _dir: String,
                _url: String,
                _token: String,
                _verify_blob: bool,
                _blob_sum: String,
            ) -> Result<(), MirrorError> {
                info!("testing logging in fake test");
                Ok(())
            }
        }

        let fake = Fake {};

        let ops = vec![op.clone()];
        let res = aw!(get_operator_catalog(
            fake.clone(),
            String::from("./test-artifacts/"),
            false,
            false,
            ops.clone()
        ));
        println!("result -> {}", res.is_ok());
    }
}
