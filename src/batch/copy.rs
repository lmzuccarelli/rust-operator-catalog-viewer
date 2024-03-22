// module copy

use async_trait::async_trait;
use futures::{stream, StreamExt};
use reqwest::Client;
use std::collections::HashSet;
use std::fs;
use std::path::Path;

use crate::api::schema::*;
use crate::error::handler::*;
use crate::log::logging::*;

#[async_trait]
impl RegistryInterface for ImplRegistryInterface {
    async fn get_manifest(
        &self,
        url: String,
        token: String,
    ) -> Result<String, Box<dyn std::error::Error>> {
        let client = Client::new();
        // check without token
        if token.len() == 0 {
            let body = client
                .get(url)
                .header("Accept", "application/vnd.oci.image.manifest.v1+json")
                .header("Content-Type", "application/json")
                .send()
                .await?
                .text()
                .await?;

            return Ok(body);
        }

        let mut header_bearer: String = "Bearer ".to_owned();
        header_bearer.push_str(&token);
        let body = client
            .get(url)
            .header("Accept", "application/vnd.oci.image.manifest.v1+json")
            .header("Content-Type", "application/json")
            .header("Authorization", header_bearer)
            .send()
            .await?
            .text()
            .await?;

        Ok(body)
    }
    // get each blob referred to by the vector in parallel
    // set by the PARALLEL_REQUESTS value
    async fn get_blobs(
        &self,
        log: &Logging,
        dir: String,
        url: String,
        token: String,
        layers: Vec<FsLayer>,
    ) -> Result<String, Box<dyn std::error::Error>> {
        const PARALLEL_REQUESTS: usize = 16;
        let client = Client::new();

        // remove all duplicates in FsLayer
        let mut images = Vec::new();
        let mut seen = HashSet::new();
        for img in layers.iter() {
            // truncate sha256:
            let truncated_image = img.blob_sum.split(":").nth(1).unwrap();
            let inner_blobs_file = get_blobs_file(dir.clone(), &truncated_image);
            let mut exists = Path::new(&inner_blobs_file).exists();
            if exists {
                let metadata = fs::metadata(&inner_blobs_file).unwrap();
                if img.size.is_some() {
                    if metadata.len() != img.size.unwrap() as u64 {
                        exists = false;
                    }
                } else {
                    exists = false;
                }
            }

            // filter out duplicates
            if !seen.contains(&truncated_image) && !exists {
                seen.insert(truncated_image);
                if url == "" {
                    let img_orig = img.original_ref.clone().unwrap();
                    let img_ref = get_blobs_url_by_string(img_orig);
                    let layer = FsLayer {
                        blob_sum: img.blob_sum.clone(),
                        original_ref: Some(img_ref),
                        size: img.size,
                    };
                    images.push(layer);
                } else {
                    let layer = FsLayer {
                        blob_sum: img.blob_sum.clone(),
                        original_ref: Some(url.clone()),
                        size: img.size,
                    };
                    images.push(layer);
                }
            }
        }
        log.debug(&format!("blobs to download {}", images.len()));
        log.trace(&format!("fslayers vector {:#?}", images));
        let mut header_bearer: String = "Bearer ".to_owned();
        header_bearer.push_str(&token);

        if images.len() > 0 {
            log.debug("downloading blobs...");
        }

        let fetches = stream::iter(images.into_iter().map(|blob| {
            let client = client.clone();
            let url = blob.original_ref.unwrap().clone();
            let header_bearer = header_bearer.clone();
            let wrk_dir = dir.clone();

            async move {
                match client
                    .get(url.clone() + &blob.blob_sum)
                    .header("Authorization", header_bearer)
                    .send()
                    .await
                {
                    Ok(resp) => match resp.bytes().await {
                        Ok(bytes) => {
                            let blob_digest = blob.blob_sum.split(":").nth(1).unwrap();
                            let blob_dir = get_blobs_dir(wrk_dir.clone(), blob_digest);
                            fs::create_dir_all(blob_dir.clone())
                                .expect("unable to create direcory");
                            fs::write(blob_dir + &blob_digest, bytes.clone())
                                .expect("unable to write blob");
                            let msg = format!("writing blob {}", blob_digest);
                            log.info(&msg);
                        }
                        Err(_) => {
                            let msg = format!("writing blob {}", url.clone());
                            log.error(&msg);
                            //return Err(e);
                        }
                    },
                    Err(e) => {
                        // TODO: update signature to Box<dyn MirrorError>
                        // and return the error
                        //let msg = format!("downloading blob {}", &url);
                        //log.error(&msg);
                        let err = MirrorError::new(&e.to_string());
                        log.error(&err.to_string());
                    }
                }
            }
        }))
        .buffer_unordered(PARALLEL_REQUESTS)
        .collect::<Vec<()>>();
        fetches.await;
        Ok(String::from("ok"))
    }
}

// construct the blobs url
pub fn get_blobs_url(image_ref: ImageReference) -> String {
    // return a string in the form of (example below)
    // "https://registry.redhat.io/v2/redhat/certified-operator-index/blobs/";
    let mut url = String::from("https://");
    url.push_str(&image_ref.registry);
    url.push_str(&"/v2/");
    url.push_str(&image_ref.namespace);
    url.push_str("/");
    url.push_str(&image_ref.name);
    url.push_str(&"/");
    url.push_str(&"blobs/");
    url
}
// construct the blobs url by string
pub fn get_blobs_url_by_string(img: String) -> String {
    let mut parts = img.split("/");
    let mut url = String::from("https://");
    url.push_str(&parts.nth(0).unwrap());
    url.push_str(&"/v2/");
    url.push_str(&parts.nth(0).unwrap());
    url.push_str(&"/");
    let i = parts.nth(0).unwrap();
    let mut sha = i.split("@");
    url.push_str(&sha.nth(0).unwrap());
    url.push_str(&"/blobs/");
    url
}
// construct blobs dir
pub fn get_blobs_dir(dir: String, name: &str) -> String {
    // originally working-dir/blobs-store
    let mut file = dir.clone();
    file.push_str(&name[..2]);
    file.push_str(&"/");
    file
}
// construct blobs file
pub fn get_blobs_file(dir: String, name: &str) -> String {
    // originally working-dir/blobs-store
    let mut file = dir.clone();
    file.push_str("/");
    file.push_str(&name[..2]);
    file.push_str(&"/");
    file.push_str(&name);
    file
}

#[cfg(test)]
#[allow(unused_must_use)]
mod tests {
    // this brings everything from parent's scope into this scope
    use super::*;

    macro_rules! aw {
        ($e:expr) => {
            tokio_test::block_on($e)
        };
    }

    #[test]
    fn get_manifest_pass() {
        let mut server = mockito::Server::new();
        let url = server.url();

        // Create a mock
        server
            .mock("GET", "/manifests")
            .with_status(200)
            .with_header("content-type", "application/json")
            .with_body("{ \"test\": \"hello-world\" }")
            .create();

        let real = ImplRegistryInterface {};

        let res = aw!(real.get_manifest(url + "/manifests", String::from("token")));
        assert!(res.is_ok());
        assert_eq!(res.unwrap(), String::from("{ \"test\": \"hello-world\" }"));
    }

    #[test]
    fn get_blobs_pass() {
        let mut server = mockito::Server::new();
        let url = server.url();

        // Create a mock
        server
            .mock("GET", "/sha256:1234567890")
            .with_status(200)
            .with_header("content-type", "application/json")
            .with_body("{ \"test\": \"hello-world\" }")
            .create();

        let fslayer = FsLayer {
            blob_sum: String::from("sha256:1234567890"),
            original_ref: Some(url.clone()),
            size: Some(112),
        };
        let fslayers = vec![fslayer];
        let log = &Logging {
            log_level: Level::INFO,
        };

        let fake = ImplRegistryInterface {};

        // test with url set first
        aw!(fake.get_blobs(
            log,
            String::from("test-artifacts/test-blobs-store/"),
            url.clone() + "/",
            String::from("token"),
            fslayers.clone(),
        ));
        // check the file contents
        let s = fs::read_to_string("test-artifacts/test-blobs-store/12/1234567890")
            .expect("should read file");
        assert_eq!(s, "{ \"test\": \"hello-world\" }");
        fs::remove_dir_all("test-artifacts/test-blobs-store").expect("should delete");
    }

    #[test]
    fn get_blobs_file_pass() {
        let res = get_blobs_file(
            String::from("test-artifacts/index-manifest/v1/blobs-store"),
            "1234567890",
        );
        assert_eq!(
            res,
            String::from("test-artifacts/index-manifest/v1/blobs-store/12/1234567890")
        );
    }

    #[test]
    fn get_blobs_dir_pass() {
        let res = get_blobs_dir(
            String::from("test-artifacts/index-manifest/v1/blobs-store/"),
            "1234567890",
        );
        assert_eq!(
            res,
            String::from("test-artifacts/index-manifest/v1/blobs-store/12/")
        );
    }

    #[test]
    fn get_blobs_url_by_string_pass() {
        let res = get_blobs_url_by_string(String::from(
            "test.registry.io/test/some-operator@sha256:1234567890",
        ));
        assert_eq!(
            res,
            String::from("https://test.registry.io/v2/test/some-operator/blobs/")
        );
    }

    #[test]
    fn get_blobs_url_pass() {
        let pkg = Package {
            name: String::from("some-operator"),
            channels: None,
            min_version: None,
            max_version: None,
            min_bundle: None,
        };
        let pkgs = vec![pkg];
        let ir = ImageReference {
            registry: String::from("test.registry.io"),
            namespace: String::from("test"),
            name: String::from("some-operator"),
            version: String::from("v1.0.0"),
            packages: Some(pkgs),
        };
        let res = get_blobs_url(ir);
        assert_eq!(
            res,
            String::from("https://test.registry.io/v2/test/some-operator/blobs/")
        );
    }
}
