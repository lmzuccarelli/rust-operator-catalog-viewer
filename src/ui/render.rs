use crossterm::event::{self, Event, KeyCode, KeyEventKind};
use custom_logger::*;
use mirror_catalog::*;
use ratatui::widgets::ListState;
use ratatui::{prelude::*, widgets::*};
use std::collections::HashMap;
use std::process;
use std::{env, io};

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

/// set up the app state for the ui
// keep the schema and api in the same module
pub struct App {
    pub name: String,
    pub packages: StatefulList<String>,
    pub channels: StatefulList<String>,
    pub declarative_config: HashMap<String, DeclarativeConfig>,
    pub path: String,
    pub last_update: usize,
}

impl App {
    pub fn new(base_dir: String) -> Self {
        let log = Logging {
            log_level: Level::INFO,
        };
        let this_base_dir = base_dir.clone().to_owned();
        let hld_packages = DeclarativeConfig::get_packages(&this_base_dir.clone().to_string());
        let mut packages: Vec<String> = vec![];
        if hld_packages.is_err() {
            log.error("unable to get packages");
        } else {
            packages = hld_packages.unwrap();
        }
        // actaully should find the first item in the list
        // rather than hard code it
        let dc_map = DeclarativeConfig::get_declarativeconfig_map(
            this_base_dir.clone().to_string() + &"3scale-operator/updated-configs/",
        );

        // add the actual catalog of interest in the header
        // only redhat operators are supported
        let title: String;
        if base_dir.contains("redhat") {
            let catalog = base_dir.split("redhat").nth(1).unwrap();
            let catalog_name = catalog.split("/cache/").nth(0).unwrap();
            title = format!(
                "catalog viewer [ {}{} ]",
                "redhat",
                catalog_name.replace("/", ":")
            );
        } else {
            log.warn("only redhat-operators are supported");
            process::exit(1);
        }

        Self {
            name: title.clone(),
            packages: StatefulList::with_items(packages),
            channels: StatefulList::with_items(vec![]),
            declarative_config: dc_map,
            path: this_base_dir.clone(),
            last_update: 999,
        }
    }
}

/// run the app (event loop)
pub fn run_app<B: Backend>(terminal: &mut Terminal<B>, app: &mut App) -> io::Result<()> {
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

/// ui rendering
pub fn render_ui(frame: &mut Frame, app: &mut App) {
    let size = frame.size();

    let chunks = Layout::default()
        .direction(Direction::Vertical)
        .margin(2)
        .constraints(
            [
                Constraint::Length(3),
                Constraint::Min(2),
                Constraint::Length(3),
            ]
            .as_ref(),
        )
        .split(size);

    let title = Paragraph::new(app.name.as_str())
        .style(Style::default().fg(Color::Yellow))
        .alignment(Alignment::Center)
        .block(
            Block::default()
                .borders(Borders::ALL)
                .style(Style::default().fg(Color::White))
                .title("")
                .border_type(BorderType::Plain),
        );
    frame.render_widget(title, chunks[0]);

    let body = Layout::default()
        .direction(Direction::Horizontal)
        .constraints(
            [
                Constraint::Percentage(25),
                Constraint::Percentage(15),
                Constraint::Percentage(60),
            ]
            .as_ref(),
        )
        .split(chunks[1]);

    let (left, center, right) = render_complex_view(app);
    frame.render_stateful_widget(left, body[0], &mut app.packages.state.clone());
    frame.render_stateful_widget(center, body[1], &mut app.channels.state.clone());
    frame.render_widget(right, body[2]);

    let version = env!["CARGO_PKG_VERSION"];
    let name = env!["CARGO_PKG_NAME"];
    let title = format!(
        "{} {} 2024 [ use ▲ ▼  to change package,  ◄  ► to change channel/bundle, q to quit ]",
        name, version
    );

    let copyright = Paragraph::new(title.clone())
        .style(Style::default().fg(Color::Yellow))
        .alignment(Alignment::Center)
        .block(
            Block::default()
                .borders(Borders::ALL)
                .style(Style::default().fg(Color::White))
                .title("info")
                .border_type(BorderType::Plain),
        );
    frame.render_widget(copyright, chunks[2]);
}

/// render the complex view with packages, channels and bundles
fn render_complex_view<'a>(app: &mut App) -> (List<'a>, List<'a>, Table<'a>) {
    let pkg = Block::default()
        .borders(Borders::ALL)
        .style(Style::default().fg(Color::White))
        .title("packages")
        .border_type(BorderType::Plain);

    let ch_block = Block::default()
        .borders(Borders::ALL)
        .style(Style::default().fg(Color::White))
        .title("channels&bundles")
        .border_type(BorderType::Plain);

    let mut items: Vec<_> = vec![];
    for x in app.packages.items.iter() {
        items.push(ListItem::new(Line::from(vec![Span::styled(
            x.to_string(),
            Style::default(),
        )])));
    }

    let selected_id = app.packages.state.selected().unwrap();
    let pkg_name = app.packages.items[selected_id].to_string();

    if selected_id != app.last_update {
        // load the declarative_config for the given package
        let dc_map = DeclarativeConfig::get_declarativeconfig_map(
            app.path.to_string() + &pkg_name + &"/updated-configs/",
        );

        // get the relevant channels
        let mut ch_map: HashMap<String, Vec<ChannelEntry>> = HashMap::new();
        for (k, v) in dc_map.iter() {
            if k.contains("olm.channel") {
                ch_map.insert(k.clone(), v.clone().entries.unwrap());
            }
        }

        // extract bundle info
        let mut ch_items: Vec<_> = vec![];
        for (k, _v) in ch_map.iter() {
            ch_items.push(k.to_string());
            for x in ch_map.get(k).unwrap().iter() {
                ch_items.push("  ".to_owned() + &x.clone().name.clone());
            }
        }

        // update app.channel params
        app.channels.state.select(Some(0));
        app.channels.items = ch_items.clone();
        app.declarative_config = dc_map.clone();
        app.last_update = selected_id;
    }

    let mut default_channel: String = "".to_string();
    let mut ch_items: Vec<_> = vec![];

    let keys = app.declarative_config.keys();
    for k in keys {
        if k.contains("olm.package") {
            let pkg = app.declarative_config.get(k).unwrap();
            default_channel = pkg.default_channel.clone().unwrap();
            break;
        }
    }

    for x in app.channels.items.iter() {
        let name = x.clone().to_string();
        if name.contains("=olm.channel") {
            if name.contains(&default_channel) {
                ch_items.push(ListItem::new(Line::from(vec![Span::styled(
                    name,
                    Style::default()
                        .fg(Color::LightBlue)
                        .add_modifier(Modifier::BOLD),
                )])));
            } else {
                ch_items.push(ListItem::new(Line::from(vec![Span::styled(
                    name,
                    Style::default().fg(Color::White),
                )])));
            }
        } else {
            ch_items.push(ListItem::new(Line::from(vec![Span::styled(
                name,
                Style::default().fg(Color::LightYellow),
            )])));
        }
    }

    // the list has changed so update it
    let ch_list = List::new(ch_items.clone())
        .block(ch_block.clone())
        .highlight_style(
            Style::default()
                .bg(Color::Yellow)
                .fg(Color::Black)
                .add_modifier(Modifier::BOLD),
        )
        .highlight_symbol(" ");

    let pkg_list = List::new(items.clone())
        .block(pkg)
        .highlight_style(
            Style::default()
                .bg(Color::Yellow)
                .fg(Color::Black)
                .add_modifier(Modifier::BOLD),
        )
        .highlight_symbol(" ");

    // ensure we don't panic on empty items
    if app.channels.items.len() == 0 {
        let rows = vec![Row::new(vec![Cell::from(Span::styled(
            "",
            Style::default().add_modifier(Modifier::BOLD),
        ))])];
        let contraints = vec![Constraint::Length(120)];
        let header = vec![Cell::from(Span::styled(
            "",
            Style::default().add_modifier(Modifier::BOLD),
        ))];

        return (
            pkg_list,
            ch_list,
            Table::new(rows.clone(), contraints.clone())
                .header(Row::new(header.clone()))
                .block(
                    Block::default()
                        .borders(Borders::ALL)
                        .style(Style::default().fg(Color::White))
                        .title("channel&bundle detail")
                        .border_type(BorderType::Plain),
                ),
        );
    }

    let cb_selected_id = app.channels.state.selected().unwrap();
    let cb_name = app.channels.items[cb_selected_id].to_string();
    let mut rows: Vec<_> = vec![];
    let mut contraints = vec![
        Constraint::Length(55),
        Constraint::Length(65),
        Constraint::Length(65),
        Constraint::Length(120),
    ];
    let mut header = vec![
        Cell::from(Span::styled(
            "name",
            Style::default().add_modifier(Modifier::BOLD),
        )),
        Cell::from(Span::styled(
            "replaces",
            Style::default().add_modifier(Modifier::BOLD),
        )),
        Cell::from(Span::styled(
            "skip_range",
            Style::default().add_modifier(Modifier::BOLD),
        )),
        Cell::from(Span::styled(
            "skips",
            Style::default().add_modifier(Modifier::BOLD),
        )),
    ];
    let mut widths = vec![
        Constraint::Percentage(22),
        Constraint::Percentage(25),
        Constraint::Percentage(25),
        Constraint::Percentage(28),
    ];

    if cb_name.contains("=olm.channel") {
        let dc = app.declarative_config.get(&cb_name).unwrap();
        for entry in dc.entries.clone().unwrap().iter() {
            let e = entry.clone();
            let mut skips = String::from("");
            if e.skips.is_some() {
                for s in e.skips.clone().unwrap().iter() {
                    skips.push_str(&(s.to_string()));
                }
            }
            rows.push(Row::new(vec![
                Cell::from(Span::raw(e.name.to_string())),
                Cell::from(Span::raw(e.replaces.unwrap_or("".to_string()))),
                Cell::from(Span::raw(e.skip_range.unwrap_or("".to_string()))),
                Cell::from(Span::raw(skips)),
            ]));
        }
    } else {
        // this has to be a bundle
        let b_name = cb_name.clone().split("  ").nth(1).unwrap().to_owned();
        let b_name = b_name.trim().to_string() + "=olm.bundle";
        let hld_dc = app.declarative_config.get(&b_name);
        if hld_dc.is_some() {
            let dc = hld_dc.unwrap().clone();
            for bundle in dc.related_images.clone().unwrap().iter() {
                let b = bundle.clone();
                // strip the registry from the image
                //let mut splitter = b.image.splitn(2, '/');
                //let image = splitter.nth(1).unwrap().to_string();
                let name = b.name.split('/').last().unwrap();
                rows.push(Row::new(vec![
                    Cell::from(Span::raw(name.to_string())),
                    Cell::from(Span::raw(b.image.to_string())),
                ]));
            }
            contraints = vec![Constraint::Length(60), Constraint::Length(200)];
            header = vec![
                Cell::from(Span::styled(
                    "name",
                    Style::default().add_modifier(Modifier::BOLD),
                )),
                Cell::from(Span::styled(
                    "related image",
                    Style::default().add_modifier(Modifier::BOLD),
                )),
            ];
            widths = vec![Constraint::Percentage(25), Constraint::Percentage(75)];
        }
    }

    let pkg_detail = Table::new(rows.clone(), contraints.clone())
        .header(Row::new(header.clone()))
        .block(
            Block::default()
                .borders(Borders::ALL)
                .style(Style::default().fg(Color::White))
                .title("details")
                .border_type(BorderType::Plain),
        )
        .widths(widths.clone());

    (pkg_list, ch_list, pkg_detail)
}
