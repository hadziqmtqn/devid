//! TUI state and key handling. Pure: no terminal, no daemon, so it is tested directly.

use ratatui::crossterm::event::{KeyCode, KeyEvent, KeyModifiers};
use serde_json::Value;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Page {
    Sites,
    Php,
    Server,
    Logs,
    Settings,
}

impl Page {
    pub const ALL: [Page; 5] = [Page::Sites, Page::Php, Page::Server, Page::Logs, Page::Settings];

    pub fn title(self) -> &'static str {
        match self {
            Page::Sites => "Sites",
            Page::Php => "PHP",
            Page::Server => "Server",
            Page::Logs => "Logs",
            Page::Settings => "Settings",
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Focus {
    Sidebar,
    Page,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Action {
    None,
    Quit,
    Refresh,
}

/// Everything the screen shows. `status`/`sites` come from the daemon; `Err` means it is down.
pub struct App {
    pub page: Page,
    pub focus: Focus,
    /// Selected row on the current page.
    pub row: usize,
    pub show_help: bool,
    pub no_color: bool,
    pub status: Result<Value, String>,
    pub sites: Vec<Value>,
}

impl App {
    pub fn new(no_color: bool) -> Self {
        Self {
            page: Page::Sites,
            focus: Focus::Sidebar,
            row: 0,
            show_help: false,
            no_color,
            status: Err("connecting…".into()),
            sites: Vec::new(),
        }
    }

    /// Rows on the current page, for moving the selection.
    pub fn rows(&self) -> usize {
        match self.page {
            Page::Sites => self.sites.len(),
            Page::Php => self.status.as_ref().ok().and_then(|s| s["php"].as_array()).map_or(0, Vec::len),
            Page::Server => 3,
            Page::Logs | Page::Settings => 0,
        }
    }

    fn select_page(&mut self, page: Page) {
        if self.page != page {
            self.page = page;
            self.row = 0;
        }
    }

    pub fn handle_key(&mut self, key: KeyEvent) -> Action {
        if key.modifiers.contains(KeyModifiers::CONTROL) && key.code == KeyCode::Char('c') {
            return Action::Quit;
        }
        if self.show_help {
            // Any key closes help; q/esc do not also quit.
            self.show_help = false;
            return Action::None;
        }
        match key.code {
            KeyCode::Char('q') | KeyCode::Esc => return Action::Quit,
            KeyCode::Char('?') => self.show_help = true,
            KeyCode::Char('r') => return Action::Refresh,
            KeyCode::Tab | KeyCode::BackTab => {
                self.focus = if self.focus == Focus::Sidebar { Focus::Page } else { Focus::Sidebar };
            }
            KeyCode::Char(c @ '1'..='5') => {
                self.select_page(Page::ALL[c as usize - '1' as usize]);
            }
            KeyCode::Up | KeyCode::Char('k') => self.step(-1),
            KeyCode::Down | KeyCode::Char('j') => self.step(1),
            KeyCode::Enter | KeyCode::Right | KeyCode::Char('l') if self.focus == Focus::Sidebar => {
                self.focus = Focus::Page;
            }
            KeyCode::Left | KeyCode::Char('h') if self.focus == Focus::Page => self.focus = Focus::Sidebar,
            _ => {}
        }
        Action::None
    }

    fn step(&mut self, delta: isize) {
        match self.focus {
            Focus::Sidebar => {
                let i = Page::ALL.iter().position(|p| *p == self.page).unwrap_or(0) as isize;
                let next = (i + delta).clamp(0, Page::ALL.len() as isize - 1) as usize;
                self.select_page(Page::ALL[next]);
            }
            Focus::Page => {
                let rows = self.rows();
                if rows > 0 {
                    self.row = (self.row as isize + delta).clamp(0, rows as isize - 1) as usize;
                }
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use ratatui::crossterm::event::{KeyEvent, KeyModifiers};
    use serde_json::json;

    fn key(code: KeyCode) -> KeyEvent {
        KeyEvent::new(code, KeyModifiers::NONE)
    }

    #[test]
    fn navigation() {
        let mut app = App::new(false);
        assert_eq!(app.page, Page::Sites);
        app.handle_key(key(KeyCode::Down));
        app.handle_key(key(KeyCode::Char('j')));
        assert_eq!(app.page, Page::Server, "sidebar moves between pages");
        app.handle_key(key(KeyCode::Char('1')));
        assert_eq!(app.page, Page::Sites, "number keys jump to a page");
        app.handle_key(key(KeyCode::Up));
        assert_eq!(app.page, Page::Sites, "stays at the top");

        app.sites = vec![json!({"name": "a"}), json!({"name": "b"})];
        app.handle_key(key(KeyCode::Enter));
        assert_eq!(app.focus, Focus::Page);
        for _ in 0..5 {
            app.handle_key(key(KeyCode::Down));
        }
        assert_eq!(app.row, 1, "row selection is clamped to the list");
        app.handle_key(key(KeyCode::Tab));
        assert_eq!(app.focus, Focus::Sidebar);
        app.handle_key(key(KeyCode::Char('2')));
        assert_eq!((app.page, app.row), (Page::Php, 0), "changing page resets the row");
    }

    #[test]
    fn help_quit_refresh() {
        let mut app = App::new(false);
        assert_eq!(app.handle_key(key(KeyCode::Char('?'))), Action::None);
        assert!(app.show_help);
        assert_eq!(app.handle_key(key(KeyCode::Char('q'))), Action::None, "q closes help first");
        assert!(!app.show_help);
        assert_eq!(app.handle_key(key(KeyCode::Char('r'))), Action::Refresh);
        assert_eq!(app.handle_key(key(KeyCode::Char('q'))), Action::Quit);
        assert_eq!(app.handle_key(KeyEvent::new(KeyCode::Char('c'), KeyModifiers::CONTROL)), Action::Quit);
    }
}
