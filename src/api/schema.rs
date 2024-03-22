use async_trait::async_trait;
use clap::Parser;
use ratatui::widgets::ListState;
use serde::de::{self, MapAccess, Visitor};
use serde::{Deserialize, Deserializer};
use serde_derive::{Deserialize, Serialize};
use std::fmt;
use std::marker::PhantomData;
use std::str::FromStr;
use void::Void;

use crate::log::logging::*;

/// rust-container-tool cli struct
#[derive(Parser, Debug)]
#[command(name = "rust-operator-catalog-viewer")]
#[command(author = "Luigi Mario Zuccarelli <luzuccar@redhat.com>")]
#[command(version = "0.0.1")]
#[command(about = "Used to view redhat specific operator catalogs", long_about = None)]
#[command(author, version, about, long_about = None)]
pub struct Cli {
    /// config file to use
    #[arg(short, long, value_name = "config", default_value = "")]
    pub config: Option<String>,

    /// set the loglevel. Valid arguments are info, debug, trace
    #[arg(value_enum, long, value_name = "loglevel", default_value = "info")]
    pub loglevel: Option<String>,

    #[arg(short, long, value_name = "ui", default_value = "false")]
    pub ui: Option<bool>,

    #[arg(short, long, value_name = "ui", default_value = "")]
    pub base_dir: Option<String>,

    #[arg(short, long, value_name = "dev-enable", default_value = "false")]
    pub dev_enable: Option<bool>,

    // used with dev_enable to test
    #[arg(short, long, value_name = "operator", default_value = "")]
    pub operator: Option<String>,
}

#[derive(Debug, Clone)]
pub struct StatefulList<T> {
    pub state: ListState,
    pub items: Vec<T>,
}

impl<T> StatefulList<T> {
    pub fn with_items(items: Vec<T>) -> Self {
        let mut st = ListState::default();
        // set first item as selected
        st.select(Some(0));
        Self { state: st, items }
    }

    pub fn next(&mut self) {
        if self.items.len() > 0 {
            let i = match self.state.selected() {
                Some(i) => {
                    if i >= self.items.len() - 1 {
                        0
                    } else {
                        i + 1
                    }
                }
                None => 0,
            };
            self.state.select(Some(i));
        }
    }

    pub fn previous(&mut self) {
        if self.items.len() > 0 {
            let i = match self.state.selected() {
                Some(i) => {
                    if i == 0 {
                        self.items.len() - 1
                    } else {
                        i - 1
                    }
                }
                None => 0,
            };
            self.state.select(Some(i));
        }
    }
}

// schema for the declarative_config

#[derive(Serialize, Deserialize, Clone, Debug)]
#[serde(rename_all = "camelCase")]
pub struct DeclarativeConfig {
    pub schema: Option<String>,
    pub name: Option<String>,
    #[serde(rename = "defaultChannel")]
    pub default_channel: Option<String>,
    //pub icon: Option<Icon>,
    pub description: Option<String>,
    pub package: Option<String>,
    pub entries: Option<Vec<ChannelEntry>>,
    // this is adding a lot of noise
    // disabled for now
    //pub properties: Option<Vec<Property>>,
    pub image: Option<String>,
    #[serde(rename = "relatedImages")]
    pub related_images: Option<Vec<RelatedImage>>,
}

#[derive(Serialize, Deserialize, Clone, Debug)]
#[serde(rename_all = "camelCase")]
pub struct ChannelEntry {
    pub name: String,
    pub replaces: Option<String>,
    pub skips: Option<Vec<String>>,
    #[serde(rename = "skipRange")]
    pub skip_range: Option<String>,
}

#[derive(Serialize, Deserialize, Clone, Debug)]
#[serde(rename_all = "camelCase")]
pub struct RelatedImage {
    pub name: String,
    pub image: String,
}

#[derive(Serialize, Deserialize, Clone, Debug)]
#[serde(rename_all = "camelCase")]
pub struct Meta {
    pub schema: String,
    pub package: String,
    pub name: String,
}

#[derive(Serialize, Deserialize, Clone, Debug)]
#[serde(rename_all = "camelCase")]
pub struct Property {
    #[serde(rename = "type")]
    pub type_prop: String,
    #[serde(deserialize_with = "string_or_struct")]
    pub value: Value,
}

#[derive(Serialize, Deserialize, Clone, Debug)]
#[serde(rename_all = "camelCase")]
pub struct Value {
    //#[serde(rename = "type")]
    pub group: Option<String>,
    pub kind: Option<String>,
    pub version: Option<String>,
    #[serde(rename = "packageName")]
    pub package_name: Option<String>,
}

impl FromStr for Value {
    // This implementation of `from_str` can never fail, so use the impossible
    // `Void` type as the error type.
    type Err = Void;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        Ok(Value {
            group: Some(s.to_string()),
            kind: Some(s.to_string()),
            version: Some(s.to_string()),
            // This adds too much noise
            //data: Some(s.to_string()),
            package_name: Some(s.to_string()),
        })
    }
}

pub fn string_or_struct<'de, T, D>(deserializer: D) -> Result<T, D::Error>
where
    T: Deserialize<'de> + FromStr<Err = Void>,
    D: Deserializer<'de>,
{
    // This is a Visitor that forwards string types to T's `FromStr` impl and
    // forwards map types to T's `Deserialize` impl. The `PhantomData` is to
    // keep the compiler from complaining about T being an unused generic type
    // parameter. We need T in order to know the Value type for the Visitor
    // impl.
    struct StringOrStruct<T>(PhantomData<fn() -> T>);

    impl<'de, T> Visitor<'de> for StringOrStruct<T>
    where
        T: Deserialize<'de> + FromStr<Err = Void>,
    {
        type Value = T;

        fn expecting(&self, formatter: &mut fmt::Formatter) -> fmt::Result {
            formatter.write_str("string or map")
        }

        fn visit_str<E>(self, value: &str) -> Result<T, E>
        where
            E: de::Error,
        {
            Ok(FromStr::from_str(value).unwrap())
        }

        fn visit_map<M>(self, map: M) -> Result<T, M::Error>
        where
            M: MapAccess<'de>,
        {
            // `MapAccessDeserializer` is a wrapper that turns a `MapAccess`
            // into a `Deserializer`, allowing it to be used as the input to T's
            // `Deserialize` implementation. T then deserializes itself using
            // the entries from the map visitor.
            Deserialize::deserialize(de::value::MapAccessDeserializer::new(map))
        }
    }
    deserializer.deserialize_any(StringOrStruct(PhantomData))
}

#[derive(Default, Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Token {
    pub token: Option<String>,
    #[serde(rename = "access_token")]
    pub access_token: Option<String>,
    #[serde(rename = "expires_in")]
    pub expires_in: Option<i64>,
    #[serde(rename = "issued_at")]
    pub issued_at: Option<String>,
}

#[derive(Default, Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Root {
    pub auths: Auths,
}

#[derive(Default, Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Auths {
    #[serde(rename = "cloud.openshift.com")]
    pub cloud_openshift_com: Option<CloudOpenshiftCom>,
    #[serde(rename = "quay.io")]
    pub quay_io: Option<QuayIo>,
    #[serde(rename = "registry.connect.redhat.com")]
    pub registry_connect_redhat_com: Option<RegistryConnectRedhatCom>,
    #[serde(rename = "registry.redhat.io")]
    pub registry_redhat_io: Option<RegistryRedhatIo>,
}

#[derive(Default, Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct CloudOpenshiftCom {
    pub auth: String,
    pub email: Option<String>,
}

#[derive(Default, Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct QuayIo {
    pub auth: String,
    pub email: Option<String>,
}

#[derive(Default, Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct RegistryConnectRedhatCom {
    pub auth: String,
    pub email: Option<String>,
}

#[derive(Default, Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct RegistryRedhatIo {
    pub auth: String,
    pub email: Option<String>,
}

#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct Image {
    #[serde(rename = "name")]
    pub name: String,
}

// ImageReference
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ImageReference {
    pub registry: String,
    pub namespace: String,
    pub name: String,
    pub version: String,
    pub packages: Option<Vec<Package>>,
}

#[derive(Serialize, Deserialize, Debug)]
pub struct Helm {}

#[derive(Serialize, Deserialize, Debug)]
pub struct Mirror {
    #[serde(rename = "platform")]
    pub platform: Option<Platform>,

    #[serde(rename = "release")]
    pub release: Option<String>,

    #[serde(rename = "operators")]
    pub operators: Option<Vec<Operator>>,

    #[serde(rename = "additionalImages")]
    pub additional_images: Option<Vec<Image>>,

    #[serde(rename = "helm")]
    pub helm: Option<Helm>,
}

#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct Operator {
    #[serde(rename = "catalog")]
    pub catalog: String,

    #[serde(rename = "packages")]
    pub packages: Option<Vec<Package>>,
}

#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct Package {
    #[serde(rename = "name")]
    pub name: String,

    #[serde(rename = "channels")]
    pub channels: Option<Vec<IncludeChannel>>,

    #[serde(rename = "minVersion")]
    pub min_version: Option<String>,

    #[serde(rename = "maxVersion")]
    pub max_version: Option<String>,

    #[serde(rename = "minBundle")]
    pub min_bundle: Option<String>,
}

#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct IncludeChannel {
    #[serde(rename = "name")]
    pub name: String,

    #[serde(rename = "minVersion")]
    pub min_version: Option<String>,

    #[serde(rename = "maxVersion")]
    pub max_version: Option<String>,

    #[serde(rename = "minBundle")]
    pub min_bundle: Option<String>,
}

#[derive(Serialize, Deserialize, Debug)]
pub struct Platform {
    #[serde(rename = "channels")]
    channels: Vec<Channel>,

    #[serde(rename = "graph")]
    graph: bool,
}

#[derive(Serialize, Deserialize, Debug)]
pub struct Channel {
    #[serde(rename = "name")]
    name: String,

    #[serde(rename = "type")]
    channel_type: String,
}

#[derive(Serialize, Deserialize, Debug)]
pub struct StorageConfig {
    #[serde(rename = "registry")]
    registry: Registry,
}

#[derive(Serialize, Deserialize, Debug)]
pub struct Registry {
    #[serde(rename = "imageURL")]
    image_url: String,

    #[serde(rename = "skipTLS")]
    skip_tls: bool,
}

#[derive(Default, Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct FsLayer {
    pub blob_sum: String,
    pub original_ref: Option<String>,
    pub size: Option<i64>,
}

// config schema
#[derive(Serialize, Deserialize, Debug)]
pub struct ImageSetConfig {
    #[serde(rename = "kind")]
    pub kind: String,

    #[serde(rename = "apiVersion")]
    pub api_version: String,

    #[serde(rename = "storageConfig")]
    pub storage_config: Option<StorageConfig>,

    #[serde(rename = "mirror")]
    pub mirror: Mirror,
}

#[derive(Default, Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Layer {
    pub media_type: String,
    pub size: i64,
    pub digest: String,
}

#[derive(Serialize, Deserialize, Clone, Debug)]
pub struct ManifestList {
    //#[serde(rename = "schemaVersion")]
    //pub schema_version: Option<i64>,
    #[serde(rename = "manifests")]
    pub manifests: Vec<Manifest>,

    #[serde(rename = "mediaType")]
    pub media_type: String,
}

#[derive(Serialize, Deserialize, Clone, Debug)]
pub struct Manifest {
    #[serde(rename = "schemaVersion")]
    pub schema_version: Option<i64>,

    #[serde(rename = "digest")]
    pub digest: Option<String>,

    #[serde(rename = "mediaType")]
    pub media_type: Option<String>,

    #[serde(rename = "platform")]
    pub platform: Option<ManifestPlatform>,

    #[serde(rename = "size")]
    pub size: Option<i64>,

    #[serde(rename = "config")]
    pub config: Option<Layer>,

    #[serde(rename = "layers")]
    pub layers: Option<Vec<Layer>>,
}

#[derive(Serialize, Deserialize, Clone, Debug)]
pub struct ManifestPlatform {
    #[serde(rename = "architecture")]
    pub architecture: String,

    #[serde(rename = "os")]
    pub os: String,
}

#[derive(Default, Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ManifestConfig {
    pub media_type: String,
    pub size: i64,
    pub digest: String,
}

// used only for operator index manifests
#[derive(Default, Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ManifestSchema {
    pub tag: Option<String>,
    pub name: Option<String>,
    pub architecture: Option<String>,
    pub schema_version: Option<i64>,
    pub config: Option<ManifestConfig>,
    pub history: Option<Vec<History>>,
    pub fs_layers: Vec<FsLayer>,
}

#[derive(Default, Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct History {
    #[serde(rename = "v1Compatibility")]
    pub v1compatibility: String,
}

// used to add path and arch (platform) info for mirroring
#[derive(Serialize, Deserialize, Clone, Debug)]
pub struct MirrorManifest {
    pub registry: String,
    pub namespace: String,
    pub name: String,
    pub version: String,
    pub component: String,
    pub channel: String,
    pub sub_component: String,
    pub manifest_file: String,
}

#[derive(Debug, Clone)]
pub struct ImplRegistryInterface {}

#[async_trait]
pub trait RegistryInterface {
    // used to interact with container registry (manifest calls)
    async fn get_manifest(
        &self,
        url: String,
        token: String,
    ) -> Result<String, Box<dyn std::error::Error>>;

    // used to interact with container registry (retrieve blobs)
    async fn get_blobs(
        &self,
        log: &Logging,
        dir: String,
        url: String,
        token: String,
        layers: Vec<FsLayer>,
    ) -> Result<String, Box<dyn std::error::Error>>;
}
