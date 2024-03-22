use crate::api::schema::*;
use crate::error::handler::*;
use crate::log::logging::*;

use base64::{engine::general_purpose, Engine as _};
use std::env;
use std::fs::File;
use std::io::Read;
use std::str;
use urlencoding::encode;

// read the credentials from set path (see podman credential reference)
pub fn get_credentials(log: &Logging) -> Result<String, Box<dyn std::error::Error>> {
    // Create a path to the desired file
    // using $XDG_RUNTIME_DIR envar
    let u = match env::var_os("XDG_RUNTIME_DIR") {
        Some(v) => v.into_string().unwrap(),
        None => {
            log.error("$XDG_RUNTIME_DIR/containers not set");
            "".to_string()
        }
    };
    // this is overkill but it ensures we exit properly
    if u.len() > 0 {
        let binding = &(u.to_owned() + "/containers/auth.json");
        // Open the path in read-only mode, returns `io::Result<File>`
        let mut file = File::open(&binding)?;
        // Read the file contents into a string, returns `io::Result<usize>`
        let mut s = String::new();
        file.read_to_string(&mut s)?;
        Ok(s)
    } else {
        Err(Box::new(MirrorError::new("$XDG_RUNTIME_DIR not set")))
    }
}

/// parse the json credentials to a struct
pub fn parse_json_creds(
    log: &Logging,
    data: String,
    mode: String,
) -> Result<String, Box<dyn std::error::Error>> {
    // parse the string of data into serde_json::Root.
    let creds: Root = serde_json::from_str(&data)?;
    if mode == "quay.io" {
        log.trace("using credentials for quay.io`");
        return Ok(creds.auths.quay_io.unwrap().auth);
    }
    log.trace("using credentials for registry.redhat.io");
    Ok(creds.auths.registry_redhat_io.unwrap().auth)
}

/// parse the json from the api call
pub fn parse_json_token(data: String, mode: String) -> Result<String, Box<dyn std::error::Error>> {
    // parse the string of data into serde_json::Token.
    let root: Token = serde_json::from_str(&data)?;
    if &mode == "quay.io" {
        return Ok(root.token.unwrap());
    } else {
        return Ok(root.access_token.unwrap());
    }
}

/// update quay.io account and urlencode
fn update_url(mut url: String, account: String) -> String {
    let mut result = String::from("https://");
    let service = "quay%2Eio";
    let scope = "repository%3Aopenshift-release-dev%2Focp-v4.0-art-dev%3Apull";
    let account_encoded = encode(&account).to_string();
    url.push_str(&("account=".to_owned() + &account_encoded));
    url.push_str(&("&service=".to_owned() + &service));
    url.push_str(&("&scope=".to_owned() + &scope));
    result.push_str(&url);
    result
}

/// async api call with basic auth
pub async fn get_auth_json(
    url: String,
    user: String,
    password: String,
) -> Result<String, Box<dyn std::error::Error>> {
    let client = reqwest::Client::new();
    let pwd: Option<String> = Some(password);
    let body = client
        .get(&url)
        .basic_auth(user, pwd)
        .header(
            "User-Agent",
            "containers/5.29.1-dev (github.com/containers/image)",
        )
        .send()
        .await?
        .text()
        .await?;
    Ok(body)
}

/// process all relative functions in this module to actaully get the token
pub async fn get_token(log: &Logging, name: String) -> String {
    // get creds from $XDG_RUNTIME_DIR
    let creds = get_credentials(log);
    if creds.is_err() {
        log.error(&format!("credentials {:#?}", creds.err().unwrap()));
        return "".to_string();
    }
    // parse the json data
    let rhauth = parse_json_creds(&log, creds.unwrap(), name.clone()).unwrap();
    // decode to base64
    let bytes = general_purpose::STANDARD.decode(rhauth).unwrap();

    let s = match str::from_utf8(&bytes) {
        Ok(v) => v,
        Err(e) => panic!("ERROR: invalid UTF-8 sequence: {}", e),
    };
    // get user and password form json
    let user = s.split(":").nth(0).unwrap();
    let pwd = s.split(":").nth(1).unwrap();
    let token_url = match name.as_str() {
        "registry.redhat.io" => "https://sso.redhat.com/auth/realms/rhcc/protocol/redhat-docker-v2/auth?service=docker-registry&client_id=curl&scope=repository:rhel:pull".to_string(),
        "quay.io" => {
            update_url("quay.io/v2/auth?".to_string(),user.to_string())
        },
        &_ => {
            // used for testing
            // return for the mockito server
            let mut hld = name.split("/");
            let url = hld.nth(0).unwrap();
            String::from("http://".to_string() + url + "/auth")
        },
    };
    // call the realm url to get a token with the creds
    let res = get_auth_json(token_url, user.to_string(), pwd.to_string()).await;
    let result = res.unwrap();
    // if all goes well we should have a valid token
    let token = parse_json_token(result, name.clone()).unwrap();
    token
}

#[cfg(test)]
mod tests {
    // this brings everything from parent's scope into this scope
    use super::*;
    use serial_test::serial;

    macro_rules! aw {
        ($e:expr) => {
            tokio_test::block_on($e)
        };
    }
    #[test]
    #[serial]
    fn test_get_token_redhat_pass() {
        env::remove_var("XDG_RUNTIME_DIR");
        env::set_var("XDG_RUNTIME_DIR", "/run/user/1000");
        let log = &Logging {
            log_level: Level::DEBUG,
        };
        let res = aw!(get_token(log, String::from("registry.redhat.io"),));
        assert!(res.to_string() != String::from(""));
    }
    #[test]
    #[serial]
    fn test_get_token_quay_pass() {
        env::remove_var("XDG_RUNTIME_DIR");
        env::set_var("XDG_RUNTIME_DIR", "/run/user/1000");
        let log = &Logging {
            log_level: Level::DEBUG,
        };
        let res = aw!(get_token(log, String::from("quay.io"),));
        assert!(res.to_string() != String::from(""));
    }

    #[test]
    #[serial]
    fn test_parse_json_creds_pass() {
        env::remove_var("XDG_RUNTIME_DIR");
        env::set_var("XDG_RUNTIME_DIR", "/run/user/1000");
        let log = &Logging {
            log_level: Level::DEBUG,
        };
        let data = get_credentials(log).unwrap();
        let res = parse_json_creds(log, data, String::from(""));
        assert!(res.is_ok());
    }
    #[test]
    #[serial]
    fn tst_get_credentials_pass() {
        let log = &Logging {
            log_level: Level::DEBUG,
        };
        env::remove_var("XDG_RUNTIME_DIR");
        env::set_var("XDG_RUNTIME_DIR", "/run/user/1000");
        let res = get_credentials(log);
        assert!(res.is_ok());
    }
    #[test]
    #[serial]
    fn test_get_credentials_noenvar_fail() {
        let log = &Logging {
            log_level: Level::DEBUG,
        };
        env::remove_var("XDG_RUNTIME_DIR");
        let res = get_credentials(log);
        assert!(res.is_err());
    }
    #[test]
    #[serial]
    fn test_get_credentials_nofile_fail() {
        let log = &Logging {
            log_level: Level::DEBUG,
        };
        env::remove_var("XDG_RUNTIME_DIR");
        env::set_var("XDG_RUNTIME_DIR", "/run/");
        let res = get_credentials(log);
        assert!(res.is_err());
    }
}
