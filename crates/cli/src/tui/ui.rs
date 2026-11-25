//! Rendering. D-017: the terminal's 16 colors, used only for meaning; every state also has a
//! symbol and text; NO_COLOR drops colors entirely (selection still shows through REVERSED).

use super::app::{AccountsView, App, ExtView, Focus, InputFor, Mode, Page, PickFor, Secret, human_size};
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
            // D-047: right after the server, so a narrow terminal does not cut it off.
            if st["standard_ports_ok"] == false {
                spans.push(Span::styled(" ✗ 80/443 not redirected", bad(app)));
            }
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
        Page::Mail => draw_mail(frame, app, inner),
        Page::Databases => draw_databases(frame, app, status, inner),
        Page::Logs => draw_logs(frame, app, inner),
        Page::Settings => draw_settings(frame, app, status, inner),
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
        let mut serving = vec![match s(&site["mode"]) {
            "proxy" => Span::raw(format!("proxy :{}", site["upstream"])),
            "static" => Span::raw("static"),
            _ if site["php_installed"] == true => Span::raw(format!(
                "PHP {}{}",
                s(&site["php"]),
                if site["isolated"] == true { " (isolated)" } else { "" }
            )),
            _ => Span::styled(format!("✗ PHP {} not installed", s(&site["php"])), bad(app)),
        }];
        if let Some(port) = site["port"].as_u64() {
            match site["port_busy"].is_string() {
                true => serving.push(Span::styled(format!(" · :{port} busy"), warn(app))),
                false => serving.push(Span::raw(format!(" · :{port}"))),
            }
        }
        let running = site["commands"].as_array().into_iter().flatten().filter(|c| c["state"] == "running").count();
        // D-054: an on-demand site sleeps until its next visit. The column is narrow: "on demand"
        // shows when that command is the only one running (the commands pick marks it otherwise).
        let on_demand = site["on_demand"]["state"].as_str();
        match (running, on_demand) {
            (0, _) => {}
            (1, Some("running" | "starting")) => serving.push(Span::styled(" · ● on demand", ok(app))),
            _ => serving.push(Span::styled(format!(" · ● {running} running"), ok(app))),
        }
        match on_demand {
            Some("sleeping") => serving.push(Span::raw(" · sleeping")),
            Some("failed") => serving.push(Span::styled(" · on demand, exited", warn(app))),
            _ => {}
        }
        if site["share"]["url"].is_string() {
            serving.push(Span::styled(" · shared", ok(app)));
        }
        Row::new(vec![
            Cell::from(s(&site["url"]).to_string()),
            Cell::from(site["preset_label"].as_str().unwrap_or_default().to_string()),
            Cell::from(Line::from(serving)),
            Cell::from(tilde(s(&site["path"]))),
        ])
        .style(row_style(app, i))
    });
    let widths = [Constraint::Percentage(34), Constraint::Length(14), Constraint::Length(34), Constraint::Min(10)];
    let table = Table::new(rows, widths)
        .header(Row::new(["URL", "Type", "Serving", "Folder"]).style(Style::default().add_modifier(Modifier::BOLD)));
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
    let [list, info] = Layout::vertical([Constraint::Length(5), Constraint::Min(2)]).areas(area);
    let rows = app.servers.iter().enumerate().map(|(i, srv)| {
        let state_text = if srv["paused"] == true { "stopped".to_string() } else { s(&srv["status"]).to_string() };
        Row::new(vec![
            Cell::from(format!("{} {}", if srv["active"] == true { "*" } else { " " }, s(&srv["server"]))),
            Cell::from(state(app, &state_text)),
            Cell::from(srv["version"].as_str().unwrap_or("").to_string()),
            Cell::from(if srv["active"] == true { "active (one at a time, D-005)" } else { "" }),
        ])
        .style(row_style(app, i))
    });
    frame.render_widget(
        Table::new(rows, [Constraint::Length(9), Constraint::Length(17), Constraint::Length(9), Constraint::Min(10)])
            .header(Row::new(["Server", "Status", "Version", ""]).style(Style::default().add_modifier(Modifier::BOLD))),
        list,
    );
    let lines = vec![
        Line::raw(format!(
            "Listening on 127.0.0.1:{} (http) and :{} (https){}",
            status["http_port"],
            status["https_port"],
            if status["standard_ports_ok"] == false {
                format!("; ✗ 80/443 do not reach it: {}", crate::setup::REDIRECT_FIX)
            } else if status["standard_ports"] == true {
                "; reached on 80/443 through the pf redirect".to_string()
            } else {
                String::new()
            }
        )),
        Line::raw(format!("DNS: {}", s(&status["dns"]))),
    ];
    frame.render_widget(Paragraph::new(lines).wrap(Wrap { trim: false }), info);
}

fn draw_logs(frame: &mut Frame, app: &App, area: Rect) {
    if app.logs.is_empty() {
        frame.render_widget(Paragraph::new("No logs yet."), area);
        return;
    }
    let list_height = (app.logs.len() as u16 + 1).min(area.height / 3).max(2);
    let [list, text] = Layout::vertical([Constraint::Length(list_height), Constraint::Min(1)]).areas(area);
    let rows = app.logs.iter().enumerate().map(|(i, l)| {
        Row::new(vec![Cell::from(s(&l["name"]).to_string()), Cell::from(format!("{} bytes", l["size"]))])
            .style(row_style(app, i))
    });
    frame.render_widget(
        Table::new(rows, [Constraint::Length(26), Constraint::Min(10)]).header(
            Row::new(["Log", if app.follow { "Size · following" } else { "Size" }])
                .style(Style::default().add_modifier(Modifier::BOLD)),
        ),
        list,
    );
    let block = Block::new().borders(Borders::TOP);
    let inner = block.inner(text);
    frame.render_widget(block, text);
    // A progress bar (cargo, npm) redraws its line after each \r; show what a terminal would.
    let lines: Vec<&str> =
        app.log_text.lines().map(|l| l.trim_end_matches('\r').rsplit('\r').next().unwrap_or_default()).collect();
    let visible = &lines[lines.len().saturating_sub(inner.height as usize)..];
    let body = if lines.is_empty() {
        vec![Line::styled("(empty)", Style::default().add_modifier(Modifier::DIM))]
    } else {
        visible.iter().map(|l| Line::raw(l.to_string())).collect()
    };
    frame.render_widget(Paragraph::new(body), inner);
}

/// Mail (D-049): what Mailpit does now, and how an app sends to it.
fn draw_mail(frame: &mut Frame, app: &App, area: Rect) {
    let m = app.mail();
    let dim = Style::default().add_modifier(Modifier::DIM);
    let lines = if m.is_null() {
        vec![Line::styled("This daemon has no Mail yet; update Devid.", dim)]
    } else if m["enabled"] != true {
        let first = if m["installed"] == true { "" } else { " The first time, Devid downloads Mailpit (11 MB)." };
        vec![
            Line::raw("Mail is off. It catches the email your apps send, in an inbox on this computer."),
            Line::raw(format!("Press m to turn it on.{first}")),
        ]
    } else {
        let state = s(&m["state"]);
        let style = match state {
            "running" => ok(app),
            "failed" | "not installed" => bad(app),
            _ => Style::default(),
        };
        let pid = m["pid"].as_u64().map(|p| format!(", pid {p}")).unwrap_or_default();
        let port = &m["port"];
        let mut lines = vec![
            Line::styled(format!("Mail:      {state} (Mailpit {}{pid})", s(&m["version"])), style),
            Line::raw(format!("SMTP:      127.0.0.1:{port}, any login, no TLS")),
            Line::raw(format!(".env:      MAIL_MAILER=smtp MAIL_HOST=127.0.0.1 MAIL_PORT={port}")),
            Line::raw(format!("Inbox:     {}", s(&m["inbox_url"]))),
            Line::raw(match (m["total"].as_u64(), m["unread"].as_u64()) {
                (Some(total), Some(unread)) => {
                    format!("Messages:  {total}, {unread} unread (keeps the newest {})", m["keep"])
                }
                _ => format!("Keeps:     the newest {} messages", m["keep"]),
            }),
            Line::raw(format!("PHP mail(): {}", if m["php_mail"] == true { "goes to Mail" } else { "not caught" })),
        ];
        if let Some(e) = m["error"].as_str() {
            lines.insert(1, Line::styled(format!("Problem:   {e}"), bad(app)));
        }
        lines.push(Line::raw(""));
        lines.push(Line::styled("Port and how many to keep: `devid mail port <n>`, `devid mail keep <n>`.", dim));
        lines
    };
    frame.render_widget(Paragraph::new(lines).wrap(Wrap { trim: false }), area);
}

/// Databases (D-061): each engine's server, then the selected one's address, main account and data.
fn draw_databases(frame: &mut Frame, app: &App, status: &Value, area: Rect) {
    let list = app.databases();
    let dim = Style::default().add_modifier(Modifier::DIM);
    if list.is_empty() {
        frame.render_widget(Paragraph::new(Line::styled("This daemon has no databases yet; update Devid.", dim)), area);
        return;
    }
    let [table, info] = Layout::vertical([Constraint::Length(list.len() as u16 + 2), Constraint::Min(2)]).areas(area);
    let rows = list.iter().enumerate().map(|(i, d)| {
        let on = d["enabled"] == true;
        let series = match (d["series"].as_str(), d["version"].as_str()) {
            (Some(series), Some(version)) if on => format!("{series} ({version})"),
            (Some(series), None) if on => series.to_string(),
            _ => "-".to_string(),
        };
        let port = d["port"].as_u64().filter(|_| on).map(|p| format!("127.0.0.1:{p}")).unwrap_or_default();
        Row::new(vec![
            Cell::from(s(&d["label"]).to_string()),
            Cell::from(series),
            Cell::from(state(app, s(&d["state"]))),
            Cell::from(port),
        ])
        .style(row_style(app, i))
    });
    frame.render_widget(
        Table::new(rows, [Constraint::Length(12), Constraint::Length(16), Constraint::Length(17), Constraint::Min(10)])
            .header(
                Row::new(["Database", "Series", "Status", "Address"])
                    .style(Style::default().add_modifier(Modifier::BOLD)),
            ),
        table,
    );
    let d = &list[app.row.min(list.len() - 1)];
    let (engine, label) = (s(&d["engine"]), s(&d["label"]));
    let mut lines = Vec::new();
    if d["enabled"] != true {
        lines.push(Line::raw(format!("{label} is off. Enter starts it; the first time, Devid downloads it.")));
    } else {
        let main = devid_daemon::db::Engine::parse(engine).map_or("root", devid_daemon::db::main_account);
        let password = match engine {
            "mongodb" => "its password is kept by Devid (Accounts… copies it)",
            _ => "no password from this computer",
        };
        lines.push(Line::raw(format!("Main account: {main}, {password}")));
        if let Some(sock) = d["socket"].as_str() {
            lines.push(Line::raw(format!("Socket:       {}", tilde(sock))));
        }
        let data = format!("{}/data/{engine}-{}", s(&status["home"]), s(&d["series"]));
        lines.push(Line::raw(format!("Data:         {} (kept when you stop or uninstall)", tilde(&data))));
        let installed: Vec<&str> = d["installed"].as_array().into_iter().flatten().filter_map(|v| v.as_str()).collect();
        if installed.len() > 1 {
            lines.push(Line::raw(format!("Installed:    {}", installed.join(", "))));
        }
        if let Some(e) = d["error"].as_str() {
            lines.push(Line::styled(format!("Problem:      {e}"), bad(app)));
        }
    }
    lines.push(Line::raw(""));
    lines.push(Line::styled("Another series: `devid db use <engine> <series>` (each has its own data).", dim));
    frame.render_widget(Paragraph::new(lines).wrap(Wrap { trim: false }), info);
}

fn draw_settings(frame: &mut Frame, app: &App, status: &Value, area: Rect) {
    let mut lines = vec![
        Line::raw(format!("Domain:    *.{}", s(&status["tld"]))),
        Line::raw(format!(
            "Ports:     http {}, https {}, dns {}",
            status["http_port"], status["https_port"], status["dns_port"]
        )),
        Line::raw(format!("Storage:   {}", tilde(s(&status["home"])))),
        Line::raw(format!("CA:        {}", tilde(s(&status["ca"])))),
        Line::raw(format!("devid:     {}", s(&status["version"]))),
        Line::raw(format!(
            "At login:  sites {} (a toggles) · app {} (`devid login-item on|off`)",
            if status["autostart"] == false { "wait for start all" } else { "served" },
            if status["login_item"].is_string() { "opens" } else { "does not open" }
        )),
        Line::raw(format!(
            "Terminal:  {} (p toggles)",
            if status["terminal"]["on"] == true {
                "php, php8.x, composer, nginx, caddy and mailpit are on PATH"
            } else {
                "Devid's commands are not on PATH"
            }
        )),
        Line::raw(format!(
            "Updates:   {}",
            crate::updates_line(&status["updates"]).unwrap_or_else(|| "not checked yet (every few minutes)".into())
        )),
        Line::raw(""),
        Line::raw(
            "d doctor · u check updates · U update PHP/nginx · D update devid · a serve at login · p PATH · s setup · X uninstall",
        ),
        Line::raw(""),
    ];
    if app.checks.is_empty() {
        lines.push(Line::styled("Press d to run the health check.", Style::default().add_modifier(Modifier::DIM)));
    }
    for (good, what, fix) in &app.checks {
        if *good {
            lines.push(Line::styled(format!("ok  {what}"), ok(app)));
        } else {
            lines.push(Line::styled(format!("✗   {what}"), bad(app)));
            lines.push(Line::raw(format!("    fix: {fix}")));
        }
    }
    frame.render_widget(Paragraph::new(lines).wrap(Wrap { trim: false }), area);
}

/// The last action's result if any, otherwise the keys for this page.
fn draw_footer(frame: &mut Frame, app: &App, area: Rect) {
    let line = match &app.message {
        Some((text, true)) => Line::styled(format!(" ✗ {text}"), bad(app)),
        Some((text, false)) => Line::styled(format!(" ✓ {text}"), ok(app)),
        None => {
            let keys = match (app.page, app.focus) {
                (Page::Sites, Focus::Page) => {
                    " enter actions · a link folder · p park folder · U unpark · I unignore · tab sidebar · ? help · q quit"
                }
                (Page::Server, _) => {
                    " enter actions (use, install, restart, stop/start, view config) · S stop all · A start all · ? help · q quit"
                }
                (Page::Settings, _) => {
                    " d doctor · u check updates · U update PHP/nginx · D update devid · a serve at login · p PATH · s setup · X uninstall · ? help · q quit"
                }
                (Page::Logs, _) => " ↑↓ pick a log · f follow · c clear · tab sidebar · ? help · q quit",
                (Page::Mail, _) => {
                    " m on/off · t test email · o open inbox · c copy .env · p PHP mail() · X delete all · ? help · q quit"
                }
                (Page::Databases, _) => {
                    " enter actions (start/stop, accounts, copy .env, new database, port) · ? help · q quit"
                }
                (Page::Php, _) => {
                    " enter actions (extensions, default, Xdebug, pcov, php.ini, restart, install) · u check updates · U update · ? help · q quit"
                }
                (Page::Sites, Focus::Sidebar) => {
                    " ↑↓ pages · enter open · a link folder · p park folder · U unpark · I unignore · ? help · q quit"
                }
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
                Page::Server => app.servers.get(*row).map(|v| s(&v["server"]).to_string()).unwrap_or_default(),
                Page::Php => {
                    let supported = devid_daemon::php::SUPPORTED;
                    format!("PHP {}", supported[(*row).min(supported.len() - 1)])
                }
                Page::Databases => app.databases().get(*row).map(|d| s(&d["label"]).to_string()).unwrap_or_default(),
                _ => app.sites.get(*row).map(|s| s["name"].as_str().unwrap_or("?").to_string()).unwrap_or_default(),
            };
            // D-056: a Laravel site whose Vite stays on plain HTTP says what to fix, first, so a
            // short terminal cuts the hint rather than this.
            let vite = (app.page == Page::Sites)
                .then(|| app.sites.get(*row))
                .flatten()
                .filter(|site| site["vite"]["tls"] == "app_url")
                .map(|site| format!(" Vite (Safari): set APP_URL={} in .env", s(&site["vite"]["want"])));
            let width = vite.as_ref().map_or(52, |v| (v.chars().count() as u16 + 4).max(52));
            let mut lines: Vec<Line> = vite.map(|v| Line::styled(v, warn(app))).into_iter().collect();
            lines.extend(items.iter().enumerate().map(|(i, (k, label))| {
                let line = format!(" {k}  {label}");
                if i == *index { Line::styled(line, selected()) } else { Line::raw(line) }
            }));
            lines.push(Line::raw(""));
            lines.push(hint(" ↑↓ enter, or the letter · esc close"));
            let height = lines.len() as u16 + 2;
            popup(frame, width, height, &name, lines);
        }
        Mode::Input { purpose, text } => {
            let (title, label, help) = match purpose {
                InputFor::Link => {
                    ("Link a folder as <folder name>.test".to_string(), "path", " ~ works · enter confirm · esc cancel")
                }
                InputFor::Park => (
                    "Park a folder: each subfolder becomes <name>.test".to_string(),
                    "path",
                    " ~ works · enter confirm · esc cancel",
                ),
                InputFor::Port(site) => (
                    format!("Local port for {site}"),
                    "port",
                    " a port from 1024, off, or empty for the first free one · enter · esc",
                ),
                InputFor::Command(site) => (
                    format!("Add a command to {site}"),
                    "name command",
                    " e.g. dev npm run dev · enter save · esc cancel",
                ),
                InputFor::Account(engine) => (
                    format!("New {} account", label_of(engine)),
                    "name [database]",
                    " the database defaults to the name; Devid makes the password · enter · esc",
                ),
                InputFor::Database(engine) => (
                    format!("New {} database", label_of(engine)),
                    "name",
                    " lowercase letters, digits and _ · enter · esc",
                ),
                InputFor::DbPort(engine) => {
                    (format!("{} port", label_of(engine)), "port", " from 1024; it restarts there · enter · esc")
                }
            };
            popup(frame, 70, 5, &title, vec![Line::raw(format!(" {label}: {text}█")), hint(help)]);
        }
        Mode::Pick { purpose, options, index } => {
            let title = match purpose {
                PickFor::Isolate(site) => format!("PHP version for {site}"),
                PickFor::Unpark => "Unpark which folder?".to_string(),
                PickFor::Unignore => "Serve which ignored site again?".to_string(),
                PickFor::Preset(site) => format!("App type for {site}"),
                PickFor::Serve(site) => format!("Serve {site} as"),
                PickFor::Command { site, .. } => format!("Start or stop a command of {site}"),
                PickFor::Suggest { site, .. } => format!("Add a command to {site}"),
                PickFor::RemoveCommand { site, .. } => format!("Remove which command of {site}?"),
                PickFor::OnDemand { site, .. } => format!("Start which command with the first visit to {site}?"),
                PickFor::OnDemandIdle { command, .. } => format!("Stop {command} when the site is idle"),
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
            popup(frame, 72, options.len() as u16 + 3, &title, lines);
        }
        Mode::Confirm { question, .. } | Mode::ConfirmHandover { question, .. } => {
            popup(
                frame,
                60,
                5,
                "Confirm",
                vec![Line::styled(format!(" {question}"), warn(app)), hint(" y yes · n no")],
            );
        }
        Mode::Extensions(v) => draw_extensions(frame, app, v),
        Mode::Accounts(v) => draw_accounts(frame, v),
        Mode::ConfirmAccount { question, .. } => {
            popup(
                frame,
                64,
                7,
                "Confirm",
                vec![Line::styled(format!(" {question}"), warn(app)), Line::raw(""), hint(" y yes · n back")],
            );
        }
        Mode::Secret(secret) => draw_secret(frame, secret),
        Mode::ConfirmExt { question, .. } => {
            popup(
                frame,
                64,
                7,
                "Confirm",
                vec![Line::styled(format!(" {question}"), warn(app)), Line::raw(""), hint(" y yes · n back")],
            );
        }
    }
}

/// The Extensions view: catalog rows with checkboxes, the Install button under a rule, then
/// Xdebug/pcov and the bundled extensions.
fn draw_extensions(frame: &mut Frame, app: &App, v: &ExtView) {
    let hint = |t: &str| Line::styled(t.to_string(), Style::default().add_modifier(Modifier::DIM));
    let mut lines =
        vec![Line::styled(format!(" Catalog (release {})", v.release), Style::default().add_modifier(Modifier::BOLD))];
    if v.rows.is_empty() {
        lines.push(Line::raw(format!("   nothing for PHP {} in this release", v.version)));
    }
    for (i, r) in v.rows.iter().enumerate() {
        let (mark, state) = match &r.installed {
            Some(have) => (" ✓ ".to_string(), format!("installed {have}")),
            None if v.selected.contains(&r.name) => ("[x]".to_string(), format!("available  {}", human_size(r.size))),
            None => ("[ ]".to_string(), format!("available  {}", human_size(r.size))),
        };
        let text = format!(
            " {} {mark} {:<10} {:<9} {:<11} {state}",
            if i == v.index { "›" } else { " " },
            r.name,
            r.version,
            r.license
        );
        lines.push(if i == v.index {
            Line::styled(text, selected())
        } else if r.installed.is_some() {
            Line::styled(text, ok(app))
        } else {
            Line::raw(text)
        });
    }
    lines.push(Line::raw(format!(" {}", "─".repeat(66))));
    let button = format!("   [ Install selected ({}) ]", v.selected.len());
    lines.push(if v.index == v.rows.len() { Line::styled(button, selected()) } else { Line::raw(button) });
    lines.push(Line::raw(""));
    let shipped: Vec<String> =
        v.shipped.iter().map(|(k, on)| format!("{k} {}", if *on { "on" } else { "off" })).collect();
    lines.push(Line::raw(format!(" Shipped  {}", shipped.join(" · "))));
    lines.push(Line::raw(format!(" Bundled  {}", v.bundled.join(", "))));
    lines.push(Line::raw(""));
    lines.push(hint(" space tick · tab to the button · enter install the ticked ones"));
    lines.push(hint(" r remove · x Xdebug · c pcov · esc close"));
    let height = lines.len() as u16 + 2 + (v.bundled.join(", ").len() as u16 / 60);
    popup(frame, 72, height, &format!("PHP {} · Extensions", v.version), lines);
}

/// "MySQL" for "mysql".
fn label_of(engine: &str) -> String {
    devid_daemon::db::Engine::parse(engine).map_or_else(|| engine.to_string(), |e| e.label().to_string())
}

/// An engine's accounts (D-061), and the keys that act on them.
fn draw_accounts(frame: &mut Frame, v: &AccountsView) {
    let hint = |t: &str| Line::styled(t.to_string(), Style::default().add_modifier(Modifier::DIM));
    let mut lines = vec![Line::styled(
        format!("   {:<18} {:<18} Password", "Account", "Database"),
        Style::default().add_modifier(Modifier::BOLD),
    )];
    if v.rows.is_empty() {
        lines.push(Line::raw("   no accounts yet; a adds one"));
    }
    for (i, r) in v.rows.iter().enumerate() {
        let password = match r.password.as_str() {
            "kept" => "kept by Devid",
            "none" => "none from this computer",
            _ => "not known to Devid (p sets one)",
        };
        let text = format!(
            " {} {:<18} {:<18} {password}",
            if i == v.index { "›" } else { " " },
            r.name,
            r.database.as_deref().unwrap_or("-")
        );
        lines.push(if i == v.index { Line::styled(text, selected()) } else { Line::raw(text) });
    }
    lines.push(Line::raw(""));
    lines.push(hint(" a add · p new password · c copy .env · y copy password · d remove · esc close"));
    popup(frame, 80, lines.len() as u16 + 2, &format!("{} {} · Accounts", v.label, v.series), lines);
}

/// A password Devid just set, shown with the account's `.env` lines.
fn draw_secret(frame: &mut Frame, secret: &Secret) {
    let hint = |t: String| Line::styled(t, Style::default().add_modifier(Modifier::DIM));
    let mut lines = vec![
        Line::styled(format!(" Password: {}", secret.password), Style::default().add_modifier(Modifier::BOLD)),
        Line::raw(""),
    ];
    lines.extend(secret.env.iter().map(|l| Line::raw(format!(" {l}"))));
    lines.push(Line::raw(""));
    lines.push(hint(" c copy .env · y copy password · esc close".into()));
    lines.push(hint(format!(" Devid keeps it: `devid db env {} {}` shows it again", secret.engine, secret.name)));
    popup(frame, 80, lines.len() as u16 + 2, &secret.title, lines);
}

fn draw_help(frame: &mut Frame, app: &App) {
    let area = frame.area();
    let w = area.width.min(58);
    let h = area.height.min(20);
    let popup = Rect::new(area.x + (area.width - w) / 2, area.y + (area.height - h) / 2, w, h);
    let lines = vec![
        Line::raw("↑ ↓  or  j k     move"),
        Line::raw("tab              switch sidebar / page"),
        Line::raw("enter → l        open the page   ← h  back"),
        Line::raw("1 – 7            jump to a page (its number in the sidebar)"),
        Line::raw("r                refresh now (also every 2 s)"),
        Line::raw("q  esc  ctrl-c   quit"),
        Line::raw("Sites: enter = actions (open, Terminal, copy, PHP, remove, unlink)"),
        Line::raw("       a link · p park · U unpark · I unignore"),
        Line::raw("PHP:   enter = actions · u check updates · U update"),
        Line::raw("       Extensions…: space tick · enter install · r remove"),
        Line::raw("Server: enter = actions · S stop all · A start all"),
        Line::raw("Mail:  m on/off · t test · o inbox · c copy .env · p mail() · X delete"),
        Line::raw("Databases: enter = actions · Accounts…: a add · p new password"),
        Line::raw("Logs:  f follow · c clear"),
        Line::raw("Settings: d doctor · u/U updates · D devid · s setup · X uninstall"),
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
