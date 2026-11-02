//! Rendering. D-017: the terminal's 16 colors, used only for meaning; every state also has a
//! symbol and text; NO_COLOR drops colors entirely (selection still shows through REVERSED).

use super::app::{App, Focus, InputFor, Mode, Page, PickFor};
use ratatui::{
    Frame,
    layout::{Constraint, Layout, Rect},
    style::{Color, Modifier, Style},
    text::{Line, Span},
    widgets::{Block, Borders, Cell, Clear, Paragraph, Row, Table, Wrap},
};
use serde_json::Value;

fn color(app: &App, c: Color) -> Style {
    if app.no_color { Style::default() } else { Style::default().fg(c) }
}

fn ok(app: &App) -> Style {
    color(app, Color::Green)
}

fn bad(app: &App) -> Style {
    color(app, Color::Red)
}

fn warn(app: &App) -> Style {
    color(app, Color::Yellow)
}

fn selected() -> Style {
    Style::default().add_modifier(Modifier::REVERSED)
}

fn s(v: &Value) -> &str {
    v.as_str().unwrap_or("?")
}

/// "● running" / "✗ failed" / "○ stopped": symbol + text, colored by meaning.
fn state(app: &App, status: &str) -> Span<'static> {
    let (sym, style) = match status {
        "running" => ("●", ok(app)),
        "failed" | "not installed" => ("✗", bad(app)),
        "starting" => ("◐", warn(app)),
        _ => ("○", Style::default()),
    };
    Span::styled(format!("{sym} {status}"), style)
}

fn tilde(path: &str) -> String {
    match std::env::var("HOME") {
        Ok(home) if !home.is_empty() && path.starts_with(&home) => format!("~{}", &path[home.len()..]),
        _ => path.to_string(),
    }
}

pub fn draw(frame: &mut Frame, app: &App) {
    let [header, body, footer] =
        Layout::vertical([Constraint::Length(1), Constraint::Min(3), Constraint::Length(1)]).areas(frame.area());
    let [sidebar, page] = Layout::horizontal([Constraint::Length(14), Constraint::Min(20)]).areas(body);

    draw_header(frame, app, header);
    draw_sidebar(frame, app, sidebar);
    draw_page(frame, app, page);
    draw_footer(frame, app, footer);
    draw_overlay(frame, app);
    if app.show_help {
        draw_help(frame, app);
    }
}

fn draw_header(frame: &mut Frame, app: &App, area: Rect) {
    let mut spans = vec![Span::styled(" Devid ", Style::default().add_modifier(Modifier::BOLD))];
    match &app.status {
        Ok(st) => {
            spans.push(state(app, "running"));
            spans.push(Span::raw(format!("  ·  {} ", s(&st["server"]))));
            spans.push(state(app, s(&st["server_status"])));
            spans.push(Span::raw(format!(
                "  ·  PHP {} default  ·  {} sites  ·  v{}",
                s(&st["default_php"]),
                st["sites"],
                s(&st["version"])
            )));
        }
        Err(_) => spans.push(Span::styled("✗ daemon not running", bad(app))),
    }
    frame.render_widget(Paragraph::new(Line::from(spans)), area);
}

fn draw_sidebar(frame: &mut Frame, app: &App, area: Rect) {
    let lines: Vec<Line> = Page::ALL
        .iter()
        .enumerate()
        .map(|(i, p)| {
            let text = format!(" {} {:<10}", i + 1, p.title());
            if *p == app.page {
                // REVERSED shows the selection in NO_COLOR mode too; BOLD marks the focused panel.
                let style =
                    if app.focus == Focus::Sidebar { selected().add_modifier(Modifier::BOLD) } else { selected() };
                Line::styled(text, style)
            } else {
                Line::raw(text)
            }
        })
        .collect();
    frame.render_widget(Paragraph::new(lines).block(Block::new().borders(Borders::RIGHT)), area);
}

fn draw_page(frame: &mut Frame, app: &App, area: Rect) {
    let block = Block::new()
        .borders(Borders::NONE)
        .title(Line::styled(format!(" {} ", app.page.title()), Style::default().add_modifier(Modifier::BOLD)));
    let inner = block.inner(area);
    frame.render_widget(block, area);

    let status = match &app.status {
        Ok(st) => st,
        Err(e) => {
            let text = vec![
                Line::styled("The Devid daemon is not running.", bad(app)),
                Line::raw(""),
                Line::raw("Start it with:  devid setup        (installs and starts the LaunchAgent)"),
                Line::raw("or run once:    devid daemon run"),
                Line::raw(""),
                Line::styled(format!("({e})"), Style::default().add_modifier(Modifier::DIM)),
            ];
            frame.render_widget(Paragraph::new(text).wrap(Wrap { trim: false }), inner);
            return;
        }
    };
    match app.page {
        Page::Sites => draw_sites(frame, app, inner),
        Page::Php => draw_php(frame, app, status, inner),
        Page::Server => draw_server(frame, app, status, inner),
        Page::Logs => frame.render_widget(
            Paragraph::new(vec![
                Line::raw("Log viewer (tail, follow, clear) arrives in T4."),
                Line::raw(format!("Logs are in {}/log/", tilde(s(&status["home"])))),
            ]),
            inner,
        ),
        Page::Settings => draw_settings(frame, status, inner),
    }
}

fn row_style(app: &App, i: usize) -> Style {
    if app.focus == Focus::Page && i == app.row { selected() } else { Style::default() }
}

fn draw_sites(frame: &mut Frame, app: &App, area: Rect) {
    if app.sites.is_empty() {
        frame.render_widget(
            Paragraph::new("No sites yet. Park a folder with `devid park ~/Projects` or link one with `devid link`."),
            area,
        );
        return;
    }
    let rows = app.sites.iter().enumerate().map(|(i, site)| {
        let php = if site["php_installed"] == true {
            Span::raw(format!("PHP {}{}", s(&site["php"]), if site["isolated"] == true { " (isolated)" } else { "" }))
        } else {
            Span::styled(format!("✗ PHP {} not installed", s(&site["php"])), bad(app))
        };
        Row::new(vec![Cell::from(s(&site["url"]).to_string()), Cell::from(php), Cell::from(tilde(s(&site["path"])))])
            .style(row_style(app, i))
    });
    let table = Table::new(rows, [Constraint::Percentage(40), Constraint::Length(26), Constraint::Min(10)])
        .header(Row::new(["URL", "PHP", "Folder"]).style(Style::default().add_modifier(Modifier::BOLD)));
    frame.render_widget(table, area);
}

fn draw_php(frame: &mut Frame, app: &App, _status: &Value, area: Rect) {
    let on_off = |p: &Value, e: &str| {
        let on = p["extensions"][e] == true;
        Span::styled(format!("{e} {}", if on { "●" } else { "○" }), if on { ok(app) } else { Style::default() })
    };
    let rows = devid_daemon::php::SUPPORTED.iter().enumerate().map(|(i, v)| {
        let cells = match app.php_entry(v) {
            Some(p) => vec![
                Cell::from(format!("PHP {v}")),
                Cell::from(state(app, s(&p["status"]))),
                Cell::from(if p["default"] == true { "default" } else { "" }),
                Cell::from(Line::from(vec![on_off(p, "xdebug"), Span::raw("  "), on_off(p, "pcov")])),
                Cell::from(p["pid"].as_u64().map(|pid| format!("pid {pid}")).unwrap_or_default()),
            ],
            None => vec![
                Cell::from(format!("PHP {v}")),
                Cell::from(Span::raw("○ not installed")),
                Cell::from(""),
                Cell::from(Span::styled("enter to install", Style::default().add_modifier(Modifier::DIM))),
                Cell::from(""),
            ],
        };
        Row::new(cells).style(row_style(app, i))
    });
    let table = Table::new(
        rows,
        [
            Constraint::Length(9),
            Constraint::Length(16),
            Constraint::Length(8),
            Constraint::Length(20),
            Constraint::Min(8),
        ],
    )
    .header(
        Row::new(["Version", "php-fpm", "", "Extensions", ""]).style(Style::default().add_modifier(Modifier::BOLD)),
    );
    frame.render_widget(table, area);
}

fn draw_server(frame: &mut Frame, app: &App, status: &Value, area: Rect) {
    let lines = vec![
        Line::from(vec![
            Span::raw(format!("Active web server: {}  ", s(&status["server"]))),
            state(app, s(&status["server_status"])),
        ]),
        Line::raw(format!(
            "Listening on 127.0.0.1:{} (http) and :{} (https){}",
            status["http_port"],
            status["https_port"],
            if status["standard_ports"] == true { "; reached on 80/443 through the pf redirect" } else { "" }
        )),
        Line::raw(format!("DNS: {}", s(&status["dns"]))),
        Line::raw(""),
        Line::raw("Switching, restart/stop/start and the config view arrive in T4."),
    ];
    frame.render_widget(Paragraph::new(lines).wrap(Wrap { trim: false }), area);
}

fn draw_settings(frame: &mut Frame, status: &Value, area: Rect) {
    let lines = vec![
        Line::raw(format!("Domain:    *.{}", s(&status["tld"]))),
        Line::raw(format!(
            "Ports:     http {}, https {}, dns {}",
            status["http_port"], status["https_port"], status["dns_port"]
        )),
        Line::raw(format!("Storage:   {}", tilde(s(&status["home"])))),
        Line::raw(format!("CA:        {}", tilde(s(&status["ca"])))),
        Line::raw(format!("devid:     {}", s(&status["version"]))),
        Line::raw(""),
        Line::raw("Updates, doctor, setup and uninstall arrive in T5."),
    ];
    frame.render_widget(Paragraph::new(lines), area);
}

/// The last action's result if any, otherwise the keys for this page.
fn draw_footer(frame: &mut Frame, app: &App, area: Rect) {
    let line = match &app.message {
        Some((text, true)) => Line::styled(format!(" ✗ {text}"), bad(app)),
        Some((text, false)) => Line::styled(format!(" ✓ {text}"), ok(app)),
        None => {
            let keys = match (app.page, app.focus) {
                (Page::Sites, Focus::Page) => {
                    " enter actions · a link folder · p park folder · U unpark · tab sidebar · ? help · q quit"
                }
                (Page::Php, _) => {
                    " enter actions (default, Xdebug, pcov, php.ini, restart, install) · u check updates · U update · ? help · q quit"
                }
                (Page::Sites, Focus::Sidebar) => {
                    " ↑↓ pages · enter open · a link folder · p park folder · U unpark · ? help · q quit"
                }
                _ => " ↑↓/jk move · tab switch panel · 1-5 pages · r refresh · ? help · q quit",
            };
            Line::styled(keys, Style::default().add_modifier(Modifier::DIM))
        }
    };
    frame.render_widget(Paragraph::new(line), area);
}

fn popup(frame: &mut Frame, width: u16, height: u16, title: &str, lines: Vec<Line>) {
    let area = frame.area();
    let (w, h) = (width.min(area.width), height.min(area.height));
    let rect = Rect::new(area.x + (area.width - w) / 2, area.y + (area.height - h) / 2, w, h);
    frame.render_widget(Clear, rect);
    frame.render_widget(
        Paragraph::new(lines).wrap(Wrap { trim: false }).block(
            Block::bordered().title(Line::styled(format!(" {title} "), Style::default().add_modifier(Modifier::BOLD))),
        ),
        rect,
    );
}

fn draw_overlay(frame: &mut Frame, app: &App) {
    let hint = |t: &str| Line::styled(t.to_string(), Style::default().add_modifier(Modifier::DIM));
    match &app.mode {
        Mode::Normal => {}
        Mode::Menu { row, items, index } => {
            let name = match app.page {
                Page::Php => format!("PHP {}", devid_daemon::php::SUPPORTED[(*row).min(2)]),
                _ => app.sites.get(*row).map(|s| s["name"].as_str().unwrap_or("?").to_string()).unwrap_or_default(),
            };
            let mut lines: Vec<Line> = items
                .iter()
                .enumerate()
                .map(|(i, (k, label))| {
                    let line = format!(" {k}  {label}");
                    if i == *index { Line::styled(line, selected()) } else { Line::raw(line) }
                })
                .collect();
            lines.push(Line::raw(""));
            lines.push(hint(" ↑↓ enter, or the letter · esc close"));
            popup(frame, 52, items.len() as u16 + 4, &name, lines);
        }
        Mode::Input { purpose, text } => {
            let title = match purpose {
                InputFor::Link => "Link a folder as <folder name>.test",
                InputFor::Park => "Park a folder: each subfolder becomes <name>.test",
            };
            popup(
                frame,
                70,
                5,
                title,
                vec![Line::raw(format!(" path: {text}█")), hint(" ~ works · enter confirm · esc cancel")],
            );
        }
        Mode::Pick { purpose, options, index } => {
            let title = match purpose {
                PickFor::Isolate(site) => format!("PHP version for {site}"),
                PickFor::Unpark => "Unpark which folder?".to_string(),
            };
            let mut lines: Vec<Line> = options
                .iter()
                .enumerate()
                .map(|(i, o)| {
                    let line = format!(" {o}");
                    if i == *index { Line::styled(line, selected()) } else { Line::raw(line) }
                })
                .collect();
            lines.push(hint(" ↑↓ enter · esc cancel"));
            popup(frame, 60, options.len() as u16 + 3, &title, lines);
        }
        Mode::Confirm { question, .. } => {
            popup(
                frame,
                60,
                5,
                "Confirm",
                vec![Line::styled(format!(" {question}"), warn(app)), hint(" y yes · n no")],
            );
        }
    }
}

fn draw_help(frame: &mut Frame, app: &App) {
    let area = frame.area();
    let w = area.width.min(58);
    let h = area.height.min(17);
    let popup = Rect::new(area.x + (area.width - w) / 2, area.y + (area.height - h) / 2, w, h);
    let lines = vec![
        Line::raw("↑ ↓  or  j k     move"),
        Line::raw("tab              switch sidebar / page"),
        Line::raw("enter → l        open the page   ← h  back"),
        Line::raw("1 – 5            jump to Sites, PHP, Server, Logs, Settings"),
        Line::raw("r                refresh now (also every 2 s)"),
        Line::raw("q  esc  ctrl-c   quit"),
        Line::raw(""),
        Line::raw("Sites: enter = actions (open, Terminal, copy, PHP, unlink)"),
        Line::raw("       a link · p park · U unpark"),
        Line::raw("PHP:   enter = actions · u check updates · U update"),
        Line::raw("Every action is also a command: `devid --help`."),
        Line::raw(if app.no_color { "NO_COLOR is set: monochrome." } else { "Set NO_COLOR=1 for a monochrome UI." }),
        Line::styled("press any key to close", Style::default().add_modifier(Modifier::DIM)),
    ];
    frame.render_widget(Clear, popup);
    frame.render_widget(
        Paragraph::new(lines)
            .block(Block::bordered().title(Line::styled(" Help ", Style::default().add_modifier(Modifier::BOLD)))),
        popup,
    );
}
