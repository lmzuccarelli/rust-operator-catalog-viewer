use clap::{Parser, Subcommand};

/// rust-container-tool cli struct
#[derive(Parser, Debug)]
#[command(name = "rust-operator-catalog-viewer")]
#[command(author = "Luigi Mario Zuccarelli <luzuccar@redhat.com>")]
#[command(version = "0.0.1")]
#[command(about = "Used to view redhat specific operator catalogs", long_about = None)]
#[command(author, version, about, long_about = None)]
pub struct Cli {
    #[command(subcommand)]
    pub command: Option<Commands>,

    /// set the loglevel. Valid arguments are info, debug, trace
    #[arg(value_enum, long, value_name = "loglevel", default_value = "info")]
    pub loglevel: Option<String>,
}

#[derive(Subcommand, Debug)]
pub enum Commands {
    /// Update subcommand (update operator cache)
    Update {
        #[arg(
            short,
            long,
            value_name = "base-dir",
            help = "Sets the directory used to share existing caches with other catalog tooling (required)"
        )]
        base_dir: String,

        /// config file to use
        #[arg(short, long, value_name = "config-file")]
        config_file: String,

        // process all architectures
        #[arg(short, long, value_name = "all-arch", default_value = "false")]
        all_arch: Option<bool>,
    },
    /// View subcommand (launches the TUI application)
    View {
        #[arg(
            short,
            long,
            value_name = "configs-dir",
            help = "The directory (location) where to find the untarred configs of speciifc the operetor (required)"
        )]
        configs_dir: String,

        #[arg(short, long, value_name = "dev-enable")]
        dev_enable: Option<bool>,

        // used with dev_enable to test
        #[arg(short, long, value_name = "operator", default_value = "")]
        operator: Option<String>,
    },
}
