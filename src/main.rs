use catalog::process::{build_updated_configs, get_declarativeconfig_map};
use clap::Parser;
use color_eyre::config::HookBuilder;
use crossterm::{
    event::{self, Event, KeyCode, KeyEventKind},
    terminal::{disable_raw_mode, enable_raw_mode, EnterAlternateScreen, LeaveAlternateScreen},
    ExecutableCommand,
};
use ratatui::prelude::*;
use std::process;
use std::{io, io::stdout};
use tokio;

// define local modules
mod api;
mod auth;
mod batch;
mod catalog;
mod config;
mod error;
mod index;
mod log;
mod manifests;
mod operator;
mod ui;

use api::schema::*;
use config::read::*;
use log::logging::*;
use operator::collector::*;
use ui::render::*;

// main entry point (use async)
#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    let args = Cli::parse();
    let cfg = args.config.as_ref().unwrap().to_string();
    let level = args.loglevel.unwrap().to_string();
    let ui = args.ui.unwrap();
    let base_dir = args.base_dir.unwrap() + "/";
    let dev_enabled = args.dev_enable.unwrap();
    let operator = args.operator.unwrap();

    // convert to enum
    let res_log_level = match level.as_str() {
        "info" => Level::INFO,
        "debug" => Level::DEBUG,
        "trace" => Level::TRACE,
        _ => Level::INFO,
    };

    // setup logging
    let log = &Logging {
        log_level: res_log_level,
    };

    if dev_enabled {
        let component = base_dir.clone() + &operator + &"/updated-configs/";
        let component_base = base_dir.clone() + &operator;
        let dc = get_declarativeconfig_map(component.clone());
        log.debug(&format!("declarative config keys {:#?}", dc.keys()));
        let res = build_updated_configs(component_base.clone());
        log.debug(&format!("updated configs {:#?}", res));
        process::exit(0);
    }

    if !ui {
        log.info(&format!("rust-operator-catalog-viewer {} ", cfg));

        // Parse the config serde_yaml::ImageSetConfiguration.
        let config = load_config(cfg).unwrap();
        let isc_config = parse_yaml_config(config.clone()).unwrap();
        log.debug(&format!(
            "image set config operators {:#?}",
            isc_config.mirror.operators
        ));

        // initialize the client request interface
        let reg_con = ImplRegistryInterface {};

        // check for release image
        if isc_config.mirror.operators.is_some() {
            get_operator_catalog(
                reg_con.clone(),
                log,
                String::from("./working-dir/"),
                isc_config.mirror.operators.unwrap(),
            )
            .await;
        }
    } else {
        init_error_hooks()?;
        let mut terminal = init_terminal()?;
        let mut app = App::new(base_dir.clone());
        let res = run_app(&mut terminal, &mut app);
        restore_terminal()?;
        if let Err(err) = res {
            println!("{err:?}");
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

/// run the app (event loop)
fn run_app<B: Backend>(terminal: &mut Terminal<B>, app: &mut App) -> io::Result<()> {
    loop {
        terminal.draw(|f| render_ui(f, app))?;
        if let Event::Key(key) = event::read()? {
            if key.kind == KeyEventKind::Press {
                use KeyCode::*;
                match key.code {
                    Char('q') | Esc => return Ok(()),
                    Down => {
                        app.packages.next();
                    }
                    Up => {
                        app.packages.previous();
                    }
                    Left => {
                        app.channels.previous();
                    }
                    Right => {
                        app.channels.next();
                    }
                    _ => {}
                }
            }
        }
    }
}
