use clap::Parser;

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

    // process all architectures
    #[arg(short, long, value_name = "all-arch", default_value = "false")]
    pub all_arch: Option<bool>,
}
