use crate::cli::config::ViewConfig;
use clap::Parser;
use color_eyre::config::HookBuilder;
use crossterm::{
    terminal::{disable_raw_mode, enable_raw_mode, EnterAlternateScreen, LeaveAlternateScreen},
    ExecutableCommand,
};
use custom_logger::*;
use mirror_catalog::DeclarativeConfig;
use mirror_config::*;
use mirror_copy::ImplDownloadImageInterface;
use mirror_error::MirrorError;
use ratatui::prelude::*;
use std::collections::HashMap;
use std::io;
use std::io::stdout;
use std::process;
use std::str::FromStr;
use tokio;

// define local modules
mod api;
mod batch;
mod cli;
mod operator;
mod ui;

use api::schema::*;
use operator::collector::*;
use ui::render::*;

// main entry point (use async)
#[allow(unused_variables)]
#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    let args = Cli::parse();

    let level = args.loglevel.as_deref().unwrap_or("info");
    let res_log_level = LevelFilter::from_str(level)
        .map_err(|_| MirrorError::new(&format!("invalid log level \"{level}\"")))?;

    // setup logging
    Logging::new()
        .with_level(res_log_level)
        .init()
        .expect("should initialize");

    match &args.command {
        Some(Commands::Update {
            working_dir,
            config_file,
            all_arch,
        }) => {
            info!("[main] operator-catalog-viewer {} ", config_file.clone());

            // Parse the config serde_yaml::ImageSetConfig.
            let config = ImageSetConfig::load_config(config_file.clone());
            if config.is_ok() {
                let isc_config =
                    ImageSetConfig::parse_yaml_config(config.unwrap().clone()).unwrap();
                debug!(
                    "[main] image set config operators {:#?}",
                    isc_config.mirror.operators
                );

                // initialize the client request interface
                let reg_con = ImplDownloadImageInterface {};

                // check for release image
                if isc_config.mirror.operators.is_some() {
                    get_operator_catalog(
                        reg_con.clone(),
                        working_dir.clone(),
                        false,
                        true,
                        isc_config.mirror.operators.unwrap(),
                    )
                    .await?;
                }
            } else {
                error!("{}", config.err().unwrap());
            }
        }
        Some(Commands::View {
            configs_dir,
            dev_enable,
            operator,
        }) => {
            if dev_enable.is_some() {
                debug!("[main] (dev-mode) operator {:?}", operator);
                if operator.is_none() {
                    error!("[main] operator flag is required use --help to get a list of flags");
                    process::exit(1);
                }
                let dir = configs_dir.as_ref().unwrap().clone();
                let op = operator.as_ref().unwrap();
                let component = dir.clone() + &op + &"/updated-configs/";
                debug!("[main] (dev-mode) op {}", op);
                debug!("[main] (dev-mode) component {}", component);

                let component_base = dir + &op;
                let dc = DeclarativeConfig::get_declarativeconfig_map(component.clone());
                debug!("[main] (dev-mode) declarative config keys {:#?}", dc.keys());
                let res = DeclarativeConfig::build_updated_configs(component_base.clone());
                debug!("[main] (dev-mode) updated configs {:#?}", res);
                process::exit(0);
            }

            let mut count = 1;
            let mut in_map: HashMap<usize, String> = HashMap::new();
            let cfg_impl = ViewConfig::new();
            let map = cfg_impl.read_config();
            info!(
                " Please select a catalog you would like to view (use the number and press enter)\n"
            );
            for (k, v) in map.iter() {
                let data = format!("{}) {}", count, k);
                println!(" {}", data);
                in_map.insert(count, k.to_lowercase());
                count += 1;
            }
            println!("");

            let mut input_line = String::new();
            io::stdin()
                .read_line(&mut input_line)
                .expect("failed to read line");

            let res = input_line.trim().parse();
            if res.is_err() {
                error!("could not parse number");
                process::exit(1);
            }
            let value = in_map.get(&res.unwrap()).unwrap();
            let configs_dir = map.get(value);

            init_error_hooks()?;
            let mut terminal = init_terminal()?;
            let mut app = App::new(value.to_lowercase(), configs_dir.unwrap().to_lowercase());
            let res = run_app(&mut terminal, &mut app);
            restore_terminal()?;
            if let Err(err) = res {
                println!("{err:?}");
            }
        }
        None => {
            error!("[main] sub command not recognized use --help to list commands");
            process::exit(1);
        }
    }
    Ok(())
}

fn init_error_hooks() -> color_eyre::Result<()> {
    let (panic, error) = HookBuilder::default().into_hooks();
    let panic = panic.into_panic_hook();
    let error = error.into_eyre_hook();
    color_eyre::eyre::set_hook(Box::new(move |e| {
        let _ = restore_terminal();
        error(e)
    }))?;
    std::panic::set_hook(Box::new(move |info| {
        let _ = restore_terminal();
        panic(info);
    }));
    Ok(())
}

fn init_terminal() -> color_eyre::Result<Terminal<impl Backend>> {
    enable_raw_mode()?;
    stdout().execute(EnterAlternateScreen)?;
    let backend = CrosstermBackend::new(stdout());
    let terminal = Terminal::new(backend)?;
    Ok(terminal)
}

fn restore_terminal() -> color_eyre::Result<()> {
    disable_raw_mode()?;
    stdout().execute(LeaveAlternateScreen)?;
    Ok(())
}
