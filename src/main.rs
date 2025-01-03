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
use ratatui::prelude::*;
use std::io::stdout;
use std::path::Path;
use std::process;
use tokio;

// define local modules
mod api;
mod batch;
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

    // convert to enum
    let mut log = &Logging {
        log_level: Level::INFO,
    };

    if args.loglevel.is_some() {
        log = match args.loglevel.as_ref().unwrap().as_str() {
            "info" => &Logging {
                log_level: Level::INFO,
            },
            "debug" => &Logging {
                log_level: Level::DEBUG,
            },
            "trace" => &Logging {
                log_level: Level::TRACE,
            },
            _ => &Logging {
                log_level: Level::INFO,
            },
        };
    }

    match &args.command {
        Some(Commands::Update {
            working_dir,
            config_file,
            all_arch,
        }) => {
            log.info(&format!(
                "[main] operator-catalog-viewer {} ",
                config_file.clone()
            ));

            // Parse the config serde_yaml::ImageSetConfig.
            let config = ImageSetConfig::load_config(config_file.clone());
            if config.is_ok() {
                let isc_config =
                    ImageSetConfig::parse_yaml_config(config.unwrap().clone()).unwrap();
                log.debug(&format!(
                    "[main] image set config operators {:#?}",
                    isc_config.mirror.operators
                ));

                // initialize the client request interface
                let reg_con = ImplDownloadImageInterface {};

                // check for release image
                if isc_config.mirror.operators.is_some() {
                    get_operator_catalog(
                        reg_con.clone(),
                        log,
                        working_dir.clone(),
                        false,
                        true,
                        isc_config.mirror.operators.unwrap(),
                    )
                    .await?;
                }
            } else {
                log.error(&format!("{}", config.err().unwrap()));
            }
        }
        Some(Commands::View {
            configs_dir,
            dev_enable,
            operator,
        }) => {
            if dev_enable.is_some() {
                log.debug(&format!("[main] (dev-mode) operator {:?}", operator));
                if operator.is_none() {
                    log.error("[main] operator flag is required use --help to get a list of flags");
                    process::exit(1);
                }
                let op = operator.as_ref().unwrap();
                let component = configs_dir.clone() + &op + &"/updated-configs/";
                log.debug(&format!("[main] (dev-mode) op {}", op));
                log.debug(&format!("[main] (dev-mode) component {}", component));

                let component_base = configs_dir.clone() + &op;
                let dc = DeclarativeConfig::get_declarativeconfig_map(component.clone());
                log.debug(&format!(
                    "[main] (dev-mode) declarative config keys {:#?}",
                    dc.keys()
                ));
                let res = DeclarativeConfig::build_updated_configs(log, component_base.clone());
                log.debug(&format!("[main] (dev-mode) updated configs {:#?}", res));
                process::exit(0);
            }
            if !Path::new(&configs_dir.clone()).exists() {
                log.error("[main] the configs directory selected does not exist");
                process::exit(1);
            }

            init_error_hooks()?;
            let mut terminal = init_terminal()?;
            let mut app = App::new(configs_dir.clone());
            let res = run_app(&mut terminal, &mut app);
            restore_terminal()?;
            if let Err(err) = res {
                println!("{err:?}");
            }
        }
        None => {
            log.error("[main] sub command not recognized use --help to list commands");
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
