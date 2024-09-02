use custom_logger::*;
use futures::stream::FuturesUnordered;
use futures::stream::StreamExt;
use mirror_auth::get_token;
use mirror_auth::ImplTokenInterface;
use mirror_copy::*;
use mirror_error::MirrorError;
use std::collections::HashMap;

pub async fn execute_batch<T: RegistryInterface + Clone>(
    reg_impl: T,
    log: &Logging,
    dir: String,
    verify_blob: bool,
    tls_verify: bool,
    map_in: HashMap<String, Vec<FsLayer>>,
) -> Result<(), MirrorError> {
    let mut futs = FuturesUnordered::new();
    let batch_size = 8;
    let bar = "% completed    [--------------------------------------------------------------]"
        .to_string();
    let t_impl = ImplTokenInterface {};
    // get blobs in batch of 8
    // each future handles get_blobs api call
    // batch the calls
    for (k, v) in map_in.clone() {
        let hld: &str;
        let mut url = k.clone();
        if url.contains("https://") {
            hld = k.split("https://").nth(1).unwrap();
            if !tls_verify {
                url = k.replace("https://", "http://");
            }
        } else {
            hld = k.split("http://").nth(1).unwrap();
        }
        // TODO: add check to see if blobs exist on disk
        let registry = hld.split("/").nth(0).unwrap();
        log.trace(&format!("url {}", k));
        let token = get_token(
            t_impl.clone(),
            log,
            registry.to_string(),
            "".to_string(),
            tls_verify,
        )
        .await?;
        let mut count = 0;
        let per_position = v.len() as f32 / 61.0;
        if v.len() > 0 {
            log.info(&format!("[execute_batch] downloading {} blobs", v.len()));
        }
        for layer in v.iter() {
            futs.push(reg_impl.get_blob(
                log,
                dir.clone(),
                url.clone(),
                token.clone(),
                verify_blob,
                layer.blob_sum.clone(),
            ));
            if futs.len() >= batch_size {
                futs.next().await.unwrap()?;
            }
            count += 1;
            if count % 10 == 0 {
                let update = count as f32 / per_position;
                let new_bar = bar.replacen("-", "#", update.floor() as usize);
                log.mid(&new_bar);
            }
        }
        // Wait for the remaining to finish.
        while let Some(response) = futs.next().await {
            response.unwrap()
        }
    }
    // Wait for the remaining to finish.
    //while let Some(response) = futs.next().await {
    //    response.unwrap()
    //}
    for (_k, v) in map_in {
        if v.len() > 0 {
            let new_bar = bar.replacen("-", "#", 62);
            log.mid(&new_bar);
            break;
        }
    }
    Ok(())
}
#[cfg(test)]
mod tests {
    // this brings everything from parent's scope into this scope
    use super::*;
    use std::fs;
    #[test]
    fn execute_batch_pass() {
        fs::create_dir_all("./test-artifacts/blobs-store/01")
            .expect("should create blobs-store test folder");

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

        server
            .mock("GET", "/v2")
            .with_status(200)
            .with_header("content-type", "application/json")
            .with_body(
                "{
                    \"blob\": \"test\",
                }",
            )
            .create();

        let log = &Logging {
            log_level: Level::INFO,
        };

        macro_rules! aw {
            ($e:expr) => {
                tokio_test::block_on($e)
            };
        }

        let mut map: HashMap<String, Vec<FsLayer>> = HashMap::new();
        let mut vec_fslayer: Vec<FsLayer> = Vec::new();
        for x in 0..30 {
            let fslayer = FsLayer {
                blob_sum: format!("sha256:0123456789ABCDEF{:0>2}", x),
                original_ref: Some(format!("{}/test/test-image", url)),
                size: Some(1234),
                number: None,
            };
            vec_fslayer.insert(0, fslayer.clone());
        }
        let no_tls_url = url.replace("http://", "https://");
        map.insert(
            format!("{}/v2/test/test-image/blobs/", no_tls_url),
            vec_fslayer.clone(),
        );
        log.hi(&format!("executing batch worker [should pass]"));
        let fake = ImplRegistryInterface {};
        let res = aw!(execute_batch(
            fake.clone(),
            log,
            "test-artifacts/".to_string(),
            false,
            false,
            map.clone()
        ));
        assert_eq!(res.is_ok(), true);
        // simulate an error
        let fslayer_err = FsLayer {
            blob_sum: format!("0123456789ABCDEF00"),
            original_ref: Some(format!("{}/test/test-image", url)),
            size: Some(1234),
            number: None,
        };
        vec_fslayer.insert(0, fslayer_err);
        map.insert(
            format!("{}/v2/test/test-image/blobs/", url),
            vec_fslayer.clone(),
        );
        log.hi(&format!("executing batch worker [should fail]"));
        let res_err = aw!(execute_batch(
            fake.clone(),
            log,
            "test-artifacts/".to_string(),
            false,
            false,
            map.clone()
        ));
        if res_err.is_err() {
            log.error(&format!(
                "result -> {}",
                res_err.as_ref().err().unwrap().to_string()
            ));
        }
        assert_eq!(res_err.is_err(), true);
        fs::remove_dir_all("./test-artifacts/blobs-store")
            .expect("should delete blobs-store test folder");
    }
}
