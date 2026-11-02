//! Rendering. D-017: the terminal's 16 colors, used only for meaning; every state also has a
//! symbol and text; NO_COLOR drops colors entirely (selection still shows through REVERSED).

use super::app::{App, Focus, Page};
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
    frame.render_widget(
        Paragraph::new(" ↑↓/jk move · tab switch panel · 1-5 pages · r refresh · ? help · q quit")
            .style(Style::default().add_modifier(Modifier::DIM)),
        footer,
    );
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

fn draw_php(frame: &mut Frame, app: &App, status: &Value, area: Rect) {
    let versions = status["php"].as_array().map(Vec::as_slice).unwrap_or_default();
    if versions.is_empty() {
        frame.render_widget(Paragraph::new("No PHP installed. Install one with `devid php install 8.5`."), area);
        return;
    }
    let rows = versions.iter().enumerate().map(|(i, p)| {
        let pid = p["pid"].as_u64().map(|pid| format!("pid {pid}")).unwrap_or_default();
        Row::new(vec![
            Cell::from(format!("PHP {}", s(&p["version"]))),
            Cell::from(state(app, s(&p["status"]))),
            Cell::from(pid),
            Cell::from(if p["default"] == true { "default" } else { "" }),
        ])
        .style(row_style(app, i))
    });
    let table =
        Table::new(rows, [Constraint::Length(10), Constraint::Length(16), Constraint::Length(12), Constraint::Min(8)]);
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

fn draw_help(frame: &mut Frame, app: &App) {
    let area = frame.area();
    let w = area.width.min(58);
    let h = area.height.min(14);
    let popup = Rect::new(area.x + (area.width - w) / 2, area.y + (area.height - h) / 2, w, h);
    let lines = vec![
        Line::raw("↑ ↓  or  j k     move"),
        Line::raw("tab              switch sidebar / page"),
        Line::raw("enter → l        open the page   ← h  back"),
        Line::raw("1 – 5            jump to Sites, PHP, Server, Logs, Settings"),
        Line::raw("r                refresh now (also every 2 s)"),
        Line::raw("q  esc  ctrl-c   quit"),
        Line::raw(""),
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
