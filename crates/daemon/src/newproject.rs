//! `devid new` (D-063, Phase 15): make a project with its framework's own tool, then serve it.
//!
//! This module holds the templates, the checks made before anything is written, the steps of each
//! template and their progress. The daemon (server.rs) runs the job: it supplies the databases
//! through [`Host`] and registers the site once the steps are done. The fixed options come from
//! N1's spike (`docs/roadmap/phase-15.md`, "N1 results").

use crate::{commands::Env, db::Engine, install, paths::Paths, php, phpext};
use serde_json::{Value, json};
use std::{
    ffi::OsString,
    fs::{self, OpenOptions},
    io::{self, Write},
    path::{Path, PathBuf},
    process::{Command, Stdio},
};

/// A template `devid new` offers.
#[derive(Debug)]
pub struct Template {
    pub key: &'static str,
    pub label: &'static str,
    /// What `--kit` takes; the first is the default (`""`: no kit).
    pub kits: &'static [&'static str],
    /// What `--db` takes; the first is the default. Empty: no database.
    pub dbs: &'static [&'static str],
    /// A Node.js template (N3): served through the proxy from its dev server.
    pub node: Option<NodeApp>,
}

/// How a Node.js template is served (D-035, D-054): its framework's package on npm (whose
/// `engines.node` is the Node it needs), the site's preset, the dev server's usual port, and the
/// command a visit starts. `$PORT` is the site's dev port, which Devid's commands get.
#[derive(Debug, Clone, Copy)]
pub struct NodeApp {
    pub framework: &'static str,
    pub preset: &'static str,
    pub port: u16,
    pub dev: &'static str,
}

const VITE: NodeApp =
    NodeApp { framework: "vite", preset: "vite", port: 5173, dev: "npm run dev -- --port $PORT --strictPort" };

pub const TEMPLATES: [Template; 15] = [
    Template { key: "php", label: "PHP (blank)", kits: &[], dbs: &[], node: None },
    Template {
        key: "laravel",
        label: "Laravel",
        kits: &["", "react", "vue", "svelte", "livewire"],
        dbs: DBS,
        node: None,
    },
    Template { key: "wordpress", label: "WordPress", kits: &[], dbs: &["mysql"], node: None },
    Template { key: "symfony", label: "Symfony", kits: &["", "skeleton"], dbs: DBS, node: None },
    Template { key: "codeigniter", label: "CodeIgniter 4", kits: &[], dbs: &[], node: None },
    Template { key: "static", label: "Static HTML", kits: &[], dbs: &[], node: None },
    Template { key: "react", label: "React (Vite)", kits: &[], dbs: &[], node: Some(VITE) },
    Template { key: "vue", label: "Vue (Vite)", kits: &[], dbs: &[], node: Some(VITE) },
    Template { key: "svelte", label: "Svelte (Vite)", kits: &[], dbs: &[], node: Some(VITE) },
    Template { key: "vite", label: "Vanilla (Vite)", kits: &[], dbs: &[], node: Some(VITE) },
    Template {
        key: "next",
        label: "Next.js",
        kits: &[],
        dbs: &[],
        node: Some(NodeApp { framework: "next", preset: "next", port: 3000, dev: "npm run dev -- -p $PORT" }),
    },
    Template {
        key: "nuxt",
        label: "Nuxt",
        kits: &[],
        dbs: &[],
        node: Some(NodeApp { framework: "nuxt", preset: "nuxt", port: 3000, dev: "npm run dev -- --port $PORT" }),
    },
    Template {
        key: "sveltekit",
        label: "SvelteKit",
        kits: &[],
        dbs: &[],
        node: Some(NodeApp { framework: "@sveltejs/kit", ..VITE }),
    },
    Template {
        key: "astro",
        label: "Astro",
        kits: &[],
        dbs: &[],
        node: Some(NodeApp {
            framework: "astro",
            preset: "node",
            port: 4321,
            // Astro 7 moves `astro dev` to the background when it detects an AI agent in its
            // environment; `--ignore-lock` keeps it in the foreground, where Devid stops it.
            dev: "npm run dev -- --port $PORT --ignore-lock",
        }),
    },
    Template {
        key: "express",
        label: "Express API",
        kits: &[],
        dbs: &[],
        node: Some(NodeApp { framework: "express", preset: "node", port: 3000, dev: "npm run dev" }),
    },
];

const DBS: &[&str] = &["sqlite", "mysql", "pgsql"];

pub fn find(key: &str) -> Result<&'static Template, String> {
    TEMPLATES.iter().find(|t| t.key == key).ok_or_else(|| {
        let known: Vec<&str> = TEMPLATES.iter().map(|t| t.key).collect();
        format!("no template {key:?}; the templates: {}", known.join(", "))
    })
}

/// Whether `v` meets an npm `engines.node` range such as `^20.19.0 || >=22.12.0` or `>= 18`.
/// Covers what frameworks write: `||`, `^`, `~`, `>=`, `>`, `<=`, `<`, `=` and partial versions.
pub fn node_satisfies(v: (u32, u32, u32), range: &str) -> bool {
    let parse = |text: &str| -> Option<(u32, u32, u32, usize)> {
        let parts: Vec<&str> = text.trim_start_matches('v').split('.').filter(|p| !p.is_empty()).collect();
        let num =
            |i: usize| parts.get(i).map_or(Some(0), |p| if *p == "x" || *p == "*" { Some(0) } else { p.parse().ok() });
        let given = parts.iter().take_while(|p| **p != "x" && **p != "*").count();
        Some((num(0)?, num(1)?, num(2)?, given))
    };
    let next_up = |(a, b, c, given): (u32, u32, u32, usize)| match given {
        0 => (u32::MAX, 0, 0),
        1 => (a + 1, 0, 0),
        2 => (a, b + 1, 0),
        _ => (a, b, c + 1),
    };
    let comparator = |op: &str, ver: &str| -> bool {
        let Some(p) = parse(ver) else { return false };
        let low = (p.0, p.1, p.2);
        match op {
            ">=" => v >= low,
            ">" => v > low,
            "<=" => v <= low,
            "<" => v < low,
            "^" => {
                let high = match p {
                    (0, 0, _, 3) => (0, 0, p.2 + 1),
                    (0, _, _, g) if g >= 2 => (0, p.1 + 1, 0),
                    _ => (p.0 + 1, 0, 0),
                };
                v >= low && v < high
            }
            "~" => v >= low && v < if p.3 >= 2 { (p.0, p.1 + 1, 0) } else { (p.0 + 1, 0, 0) },
            _ => v >= low && v < next_up(p),
        }
    };
    range.split("||").any(|clause| {
        let mut tokens = clause.split_whitespace().peekable();
        let mut all = true;
        let mut any = false;
        while let Some(token) = tokens.next() {
            let op_len = token.chars().take_while(|c| "<>=^~".contains(*c)).count();
            let (op, mut ver) = token.split_at(op_len);
            if ver.is_empty() {
                ver = tokens.next().unwrap_or_default();
            }
            any = true;
            all &= comparator(if op == "=" { "" } else { op }, ver);
        }
        any && all
    })
}

/// Laravel's starter kits build with Vite+, which needs Node 22.18+, 24.11+ or 26+ (N1).
pub const KIT_NODE: &str = "22.18+, 24.11+ or 26+";

pub fn kit_node_ok(v: (u32, u32, u32)) -> bool {
    (v.0 == 22 && v >= (22, 18, 0)) || (v.0 == 24 && v >= (24, 11, 0)) || v.0 >= 26
}

/// `v24.22.3` → (24, 22, 3).
pub fn parse_node_version(text: &str) -> Option<(u32, u32, u32)> {
    let mut parts = text.trim().trim_start_matches('v').splitn(3, '.').map(|p| p.parse::<u32>().ok());
    Some((parts.next()??, parts.next()??, parts.next()??))
}

/// What the owner asked for, checked.
#[derive(Debug, Clone, PartialEq)]
pub struct Options {
    pub template: &'static str,
    pub name: String,
    pub dir: PathBuf,
    pub php: String,
    /// The kit, or "" for the template's default.
    pub kit: String,
    /// The database engine, None for SQLite or none at all.
    pub db: Option<Engine>,
    /// SQLite (Laravel's and Symfony's default).
    pub sqlite: bool,
    /// `<name>.<tld>`
    pub host: String,
    /// `https://<host>`, with the HTTPS port unless 443 reaches Devid.
    pub url: String,
    /// Mail's SMTP port while Mail is on.
    pub mail: Option<u16>,
    /// A Node.js template's dev port: a free one from its framework's usual port.
    pub port: Option<u16>,
    /// D-065: WordPress Multisite, "subdomain" or "subdir"; None for one site.
    pub multisite: Option<&'static str>,
}

/// A project name: a site name (D-011) that also works as a database and account name once `-`
/// becomes `_`, so it starts with a letter and has at most 32 characters.
pub fn check_name(name: &str) -> Result<(), String> {
    let ok = crate::sites::valid_name(name) && name.len() <= 32 && name.starts_with(|c: char| c.is_ascii_lowercase());
    match ok {
        true => Ok(()),
        false => Err(format!(
            "{name:?} is not a project name: lowercase letters, digits and -, starting with a letter (at most 32)"
        )),
    }
}

/// The project's database and account name: its name with `-` as `_`.
pub fn db_name(name: &str) -> String {
    name.replace('-', "_")
}

/// The kit and database for a template, from `--kit` and `--db`.
pub fn choose(t: &Template, kit: Option<&str>, db: Option<&str>) -> Result<(String, Option<Engine>, bool), String> {
    let kit = match kit {
        None => String::new(),
        Some(k) if t.kits.iter().any(|x| !x.is_empty() && *x == k) => k.to_string(),
        Some(k) if t.kits.is_empty() => return Err(format!("{} has no kits (--kit {k})", t.label)),
        Some(k) => {
            let kits: Vec<&str> = t.kits.iter().copied().filter(|x| !x.is_empty()).collect();
            return Err(format!("{} has no kit {k:?}; its kits: {}", t.label, kits.join(", ")));
        }
    };
    let skeleton = t.key == "symfony" && kit == "skeleton";
    let db = match db {
        None => t.dbs.first().copied(),
        Some(_) if skeleton => return Err("Symfony's skeleton has no database layer; leave out --db".into()),
        Some(d) if t.dbs.contains(&d) => Some(d),
        Some(d) if t.dbs.is_empty() => return Err(format!("{} uses no database (--db {d})", t.label)),
        Some(d) => return Err(format!("{} takes --db {}, not {d}", t.label, t.dbs.join(", "))),
    };
    Ok(match db {
        _ if skeleton => (kit, None, false),
        Some("mysql") => (kit, Some(Engine::Mysql), false),
        Some("pgsql") => (kit, Some(Engine::Postgresql), false),
        Some("sqlite") => (kit, None, true),
        _ => (kit, None, false),
    })
}

/// D-065: `--multisite subdomain|subdir`, for WordPress only.
pub fn multisite(t: &Template, value: Option<&str>) -> Result<Option<&'static str>, String> {
    match value {
        None => Ok(None),
        Some(_) if t.key != "wordpress" => Err(format!("{} has no networks; --multisite is for WordPress", t.label)),
        Some("subdomain") => Ok(Some("subdomain")),
        Some("subdir") => Ok(Some("subdir")),
        Some(v) => Err(format!("--multisite takes subdomain or subdir, not {v}")),
    }
}

/// The project folder must not exist, or be empty; its parent must exist.
pub fn check_dir(dir: &Path) -> Result<(), String> {
    if !dir.is_absolute() {
        return Err(format!("{} is not an absolute path", dir.display()));
    }
    match fs::read_dir(dir).map(|mut entries| entries.next().is_none()) {
        Ok(false) => Err(format!("{} exists and is not empty; pick another name or --path", dir.display())),
        Ok(true) => Ok(()),
        Err(e) if e.kind() == io::ErrorKind::NotFound => match dir.parent() {
            Some(parent) if parent.is_dir() => Ok(()),
            _ => Err(format!("{} does not exist", dir.parent().unwrap_or(dir).display())),
        },
        Err(_) if dir.exists() => Err(format!("{} exists and is not a folder", dir.display())),
        Err(e) => Err(format!("{}: {e}", dir.display())),
    }
}

/// The steps of a template, in order; their labels are what the progress shows.
pub fn steps(o: &Options) -> Vec<&'static str> {
    let mut steps = Vec::new();
    match o.template {
        "php" | "static" => steps.push("Writing the first page"),
        "laravel" => {
            if !o.kit.is_empty() {
                steps.push("Checking Node.js");
            }
            if o.db.is_some() {
                steps.push("Making the database account");
            }
            steps.push(if o.kit.is_empty() {
                "Creating Laravel (composer create-project)"
            } else {
                "Creating Laravel and its starter kit (composer create-project, npm install, npm run build)"
            });
            steps.push("Setting .env: URL, database, Mail");
            if o.db.is_some() {
                steps.push("Migrating the database");
            }
        }
        "symfony" => {
            if o.db.is_some() {
                steps.push("Making the database account");
            }
            steps.push("Creating Symfony (composer create-project symfony/skeleton)");
            if o.kit.is_empty() {
                steps.push("Adding Symfony's web app (composer require webapp)");
                steps.push("Setting .env.local: database, Mail");
            }
        }
        "codeigniter" => {
            steps.push("Creating CodeIgniter 4 (composer create-project)");
            steps.push("Setting .env: environment and base URL");
        }
        t if find(t).is_ok_and(|t| t.node.is_some()) => {
            steps.push("Checking Node.js");
            steps.push(node_create_step(t));
            steps.push("Installing packages (npm install)");
        }
        "wordpress" => {
            steps.push("Getting WP-CLI");
            steps.push("Making the database account");
            steps.push("Downloading WordPress");
            steps.push("Checking WordPress's files");
            steps.push("Writing wp-config.php");
            steps.push(match o.multisite {
                Some(_) => "Installing WordPress Multisite",
                None => "Installing WordPress",
            });
        }
        _ => {}
    }
    steps.push("Serving the site");
    steps
}

fn node_create_step(template: &str) -> &'static str {
    match template {
        "next" => "Creating the Next.js app (create-next-app)",
        "nuxt" => "Creating the Nuxt app (npm create nuxt)",
        "sveltekit" => "Creating the SvelteKit app (sv create)",
        "astro" => "Creating the Astro site (npm create astro)",
        "express" => "Writing the Express app",
        _ => "Creating the Vite project (npm create vite)",
    }
}

/// One step's state, for the progress.
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum StepState {
    Pending,
    Running,
    Done,
    Failed,
}

/// The running (or last) `devid new`, for `NewProjectProgress`.
pub struct Job {
    pub template: String,
    pub name: String,
    pub path: PathBuf,
    pub log: PathBuf,
    pub steps: Vec<(String, StepState)>,
    pub error: Option<String>,
    pub done: bool,
}

impl Job {
    pub fn new(o: &Options, log: PathBuf) -> Self {
        Self {
            template: o.template.into(),
            name: o.name.clone(),
            path: o.dir.clone(),
            log,
            steps: steps(o).into_iter().map(|s| (s.to_string(), StepState::Pending)).collect(),
            error: None,
            done: false,
        }
    }

    /// Mark `label` running, and every step before it done.
    pub fn start(&mut self, label: &str) {
        let mut before = true;
        for (l, s) in &mut self.steps {
            if l == label {
                *s = StepState::Running;
                before = false;
            } else if before && *s != StepState::Done {
                *s = StepState::Done;
            }
        }
    }

    pub fn finish(&mut self, result: &Result<Value, String>) {
        self.done = true;
        match result {
            Ok(_) => self.steps.iter_mut().for_each(|(_, s)| *s = StepState::Done),
            Err(e) => {
                if let Some((_, s)) = self.steps.iter_mut().find(|(_, s)| *s == StepState::Running) {
                    *s = StepState::Failed;
                }
                self.error = Some(e.clone());
            }
        }
    }

    pub fn json(&self) -> Value {
        let state = |s: &StepState| match s {
            StepState::Pending => "pending",
            StepState::Running => "running",
            StepState::Done => "done",
            StepState::Failed => "failed",
        };
        let steps: Vec<Value> = self.steps.iter().map(|(l, s)| json!({"label": l, "state": state(s)})).collect();
        json!({
            "template": self.template,
            "name": self.name,
            "path": self.path.display().to_string(),
            "log": self.log.display().to_string(),
            "steps": steps,
            "done": self.done,
            "error": self.error,
        })
    }
}

/// What the daemon provides to a job: its databases, and the progress it shows.
pub trait Host {
    /// The engine's series and port, starting it first (and downloading it the first time).
    fn database(&self, engine: Engine) -> Result<(String, u16), String>;
    /// Make an account with a database of its own, both named `name`; returns its password.
    fn add_account(&self, engine: Engine, name: &str) -> Result<String, String>;
    /// The step that starts now.
    fn step(&self, label: &str);
}

/// What a finished job made, beyond the folder.
#[derive(Debug, Default)]
pub struct Made {
    /// The account the job made: engine, name, password.
    pub account: Option<(Engine, String, String)>,
    /// WordPress's administrator: user and password, shown once and not kept.
    pub admin: Option<(String, String)>,
    /// Commands to save for the site (D-036): name and line.
    pub commands: Vec<(String, String)>,
    pub notes: Vec<String>,
}

/// Runs the tools: Devid's PHP with its ini (D-008) for Composer, WP-CLI and artisan, everything
/// with the login shell's environment and nvm's Node (as commands get it), no stdin, and the output
/// in the job's log.
pub struct Runner {
    pub env: Env,
    pub log: PathBuf,
    pub php: PathBuf,
    pub php_env: Vec<(OsString, OsString)>,
    pub composer: PathBuf,
    pub wp: PathBuf,
}

impl Runner {
    pub fn new(paths: &Paths, version: &str, env: Env, log: PathBuf) -> Self {
        let etc = php::etc(paths, version);
        let mut php_env: Vec<(OsString, OsString)> =
            vec![("PHPRC".into(), etc.clone().into()), ("PHP_INI_SCAN_DIR".into(), etc.join("conf.d").into())];
        php_env.extend(phpext::env(paths, version).into_iter().map(|(k, v)| (k.into(), v.into())));
        php_env.push(("COMPOSER_NO_INTERACTION".into(), "1".into()));
        Self {
            env,
            log,
            php: php::dir(paths, version).join("bin/php"),
            php_env,
            composer: install::composer_phar(paths),
            wp: install::wp_cli_phar(paths),
        }
    }

    /// Run `program args` in `cwd`; on failure, the error names the step's last lines in the log.
    pub fn run(&self, cwd: &Path, program: &Path, args: &[String]) -> Result<String, String> {
        let shown: Vec<String> = args.iter().map(|a| hide_secret(a)).collect();
        self.note(&format!("$ {} {}", program.display(), shown.join(" ")));
        let log = OpenOptions::new().create(true).append(true).open(&self.log).map_err(|e| e.to_string())?;
        let out = Command::new(program)
            .args(args)
            .current_dir(cwd)
            .env_clear()
            .envs(self.env.iter().map(|(k, v)| (k, v)))
            .envs(self.php_env.iter().map(|(k, v)| (k, v)))
            .stdin(Stdio::null())
            .stdout(Stdio::piped())
            .stderr(log.try_clone().map_err(|e| e.to_string())?)
            .output()
            .map_err(|e| format!("{}: {e}", program.display()))?;
        let stdout = String::from_utf8_lossy(&out.stdout).into_owned();
        let _ = (&log).write_all(stdout.as_bytes());
        if out.status.success() {
            return Ok(stdout);
        }
        let name = program.file_name().map_or_else(String::new, |n| n.to_string_lossy().into_owned());
        let what = args.first().filter(|_| name == "php").map_or(name.clone(), |phar| {
            let phar = Path::new(phar).file_name().map_or(String::new(), |n| n.to_string_lossy().into_owned());
            match phar.as_str() {
                "composer.phar" => "composer".into(),
                "wp-cli.phar" => "wp".into(),
                _ => phar,
            }
        });
        Err(format!("{what} failed ({}); the end of {}:\n{}", out.status, self.log.display(), tail(&self.log, 8)))
    }

    pub fn composer(&self, cwd: &Path, args: &[&str]) -> Result<String, String> {
        let mut all = vec![self.composer.display().to_string()];
        all.extend(args.iter().map(|a| a.to_string()));
        self.run(cwd, &self.php, &all)
    }

    pub fn artisan(&self, dir: &Path, args: &[&str]) -> Result<String, String> {
        let mut all = vec!["artisan".to_string()];
        all.extend(args.iter().map(|a| a.to_string()));
        self.run(dir, &self.php, &all)
    }

    pub fn wp(&self, dir: &Path, args: &[String]) -> Result<String, String> {
        let mut all = vec!["-d".into(), "error_reporting=E_ALL & ~E_DEPRECATED".into(), self.wp.display().to_string()];
        all.extend(args.iter().cloned());
        self.run(dir, &self.php, &all)
    }

    pub fn npm(&self, cwd: &Path, args: &[&str]) -> Result<String, String> {
        self.run(cwd, Path::new("npm"), &args.iter().map(|a| a.to_string()).collect::<Vec<_>>())
    }

    pub fn npx(&self, cwd: &Path, args: &[&str]) -> Result<String, String> {
        self.run(cwd, Path::new("npx"), &args.iter().map(|a| a.to_string()).collect::<Vec<_>>())
    }

    /// A line of Devid's own in the log.
    pub fn note(&self, line: &str) {
        if let Ok(mut f) = OpenOptions::new().create(true).append(true).open(&self.log) {
            let _ = writeln!(f, "{line}");
        }
    }

    /// The Node on the job's PATH: its version, or why there is none.
    pub fn node(&self, cwd: &Path) -> Result<(u32, u32, u32), String> {
        let out = Command::new("node")
            .arg("--version")
            .current_dir(cwd)
            .env_clear()
            .envs(self.env.iter().map(|(k, v)| (k, v)))
            .stdin(Stdio::null())
            .output()
            .map_err(|_| "no Node.js found (nvm's default, or node on your login shell's PATH)".to_string())?;
        parse_node_version(&String::from_utf8_lossy(&out.stdout))
            .ok_or_else(|| "`node --version` printed no version".to_string())
    }
}

/// `--dbpass=…` and the like, as the log shows them.
fn hide_secret(arg: &str) -> String {
    match arg.split_once('=') {
        Some((key, _)) if key.starts_with("--") && key.contains("pass") => format!("{key}=…"),
        _ => arg.to_string(),
    }
}

/// The last `n` lines of a file.
pub fn tail(path: &Path, n: usize) -> String {
    let text = fs::read_to_string(path).unwrap_or_default();
    let lines: Vec<&str> = text.lines().filter(|l| !l.trim().is_empty()).collect();
    lines[lines.len().saturating_sub(n)..].join("\n")
}

/// How a settings file writes `KEY=value`.
#[derive(Clone, Copy)]
pub enum EnvStyle {
    /// `KEY=value` (.env of Laravel and Symfony).
    Dotenv,
    /// `KEY = value` (CodeIgniter's .env).
    Spaced,
}

/// Set keys in a settings file's text: an existing line wins, else a commented-out one (`# KEY=`)
/// is used, else the line is added at the end.
pub fn set_env(text: &str, pairs: &[(String, String)], style: EnvStyle) -> String {
    let mut lines: Vec<String> = text.lines().map(str::to_string).collect();
    let key_of = |line: &str| -> Option<(bool, String)> {
        let t = line.trim_start();
        let (commented, t) = match t.strip_prefix('#') {
            Some(rest) => (true, rest.trim_start()),
            None => (false, t),
        };
        let (key, _) = t.split_once('=')?;
        let key = key.trim();
        let ok = !key.is_empty() && key.chars().all(|c| c.is_ascii_alphanumeric() || matches!(c, '_' | '.'));
        ok.then(|| (commented, key.to_string()))
    };
    for (key, value) in pairs {
        let line = match style {
            EnvStyle::Dotenv => format!("{key}={value}"),
            EnvStyle::Spaced => format!("{key} = {value}"),
        };
        let found = |want_commented: bool| {
            lines.iter().position(|l| key_of(l).is_some_and(|(c, k)| c == want_commented && k == *key))
        };
        match found(false).or_else(|| found(true)) {
            Some(i) => lines[i] = line,
            None => lines.push(line),
        }
    }
    let mut out = lines.join("\n");
    out.push('\n');
    out
}

fn edit_env(path: &Path, pairs: &[(String, String)], style: EnvStyle) -> Result<(), String> {
    let text = match fs::read_to_string(path) {
        Ok(t) => t,
        Err(e) if e.kind() == io::ErrorKind::NotFound => String::new(),
        Err(e) => return Err(format!("{}: {e}", path.display())),
    };
    fs::write(path, set_env(&text, pairs, style)).map_err(|e| format!("{}: {e}", path.display()))
}

/// Make the project. Stops at the first failed step and leaves what was made in place.
pub fn run(o: &Options, r: &Runner, host: &dyn Host) -> Result<Made, String> {
    let parent = o.dir.parent().ok_or("the project folder has no parent")?.to_path_buf();
    let dir_arg = o.dir.display().to_string();
    let mut made = Made::default();
    match o.template {
        "php" | "static" => {
            host.step("Writing the first page");
            fs::create_dir_all(&o.dir).map_err(|e| format!("{}: {e}", o.dir.display()))?;
            let (file, body) = match o.template {
                "php" => ("index.php", format!("<?php\n\necho 'Hello from {}, on PHP ' . PHP_VERSION;\n", o.name)),
                _ => (
                    "index.html",
                    format!(
                        "<!doctype html>\n<html lang=\"en\">\n<head>\n  <meta charset=\"utf-8\">\n  \
                         <meta name=\"viewport\" content=\"width=device-width, initial-scale=1\">\n  \
                         <title>{0}</title>\n</head>\n<body>\n  <h1>{0}</h1>\n</body>\n</html>\n",
                        o.name
                    ),
                ),
            };
            fs::write(o.dir.join(file), body).map_err(|e| e.to_string())?;
        }
        "laravel" => {
            if !o.kit.is_empty() {
                host.step("Checking Node.js");
                let v = r.node(&parent)?;
                if !kit_node_ok(v) {
                    return Err(format!(
                        "Laravel's starter kits need Node.js {KIT_NODE}; this is {}.{}.{}",
                        v.0, v.1, v.2
                    ));
                }
            }
            let account = account(o, host, &mut made)?;
            let package = match o.kit.as_str() {
                "" => "laravel/laravel".to_string(),
                kit => format!("laravel/{kit}-starter-kit"),
            };
            // The kits' `dev-main` is the Laravel 13 kit, as Laravel's installer takes it (N1).
            let mut args = vec!["create-project", &package, &dir_arg, "--prefer-dist"];
            if !o.kit.is_empty() {
                args.push("--stability=dev");
            }
            host.step(steps(o).into_iter().find(|s| s.starts_with("Creating Laravel")).unwrap_or_default());
            r.composer(&parent, &args)?;
            host.step("Setting .env: URL, database, Mail");
            let mut pairs = vec![pair("APP_URL", &o.url)];
            if let Some((engine, port, password)) = &account {
                let name = db_name(&o.name);
                for line in crate::db::env_lines(*engine, *port, &name, password, Some(&name)) {
                    if let Some((k, v)) = line.split_once('=') {
                        pairs.push(pair(k, v));
                    }
                }
            }
            pairs.extend(mail_pairs(o));
            edit_env(&o.dir.join(".env"), &pairs, EnvStyle::Dotenv)?;
            if account.is_some() {
                // Laravel's own scripts made and migrated an SQLite file before the .env changed.
                let _ = fs::remove_file(o.dir.join("database/database.sqlite"));
                host.step("Migrating the database");
                r.artisan(&o.dir, &["migrate", "--force", "--no-interaction"])?;
            }
            if !o.kit.is_empty() {
                made.commands.push(("vite".into(), "npm run dev".into()));
                made.notes.push("`devid cmd start vite` runs Vite's dev server for hot reload".into());
            }
        }
        "symfony" => {
            let account = account(o, host, &mut made)?;
            host.step("Creating Symfony (composer create-project symfony/skeleton)");
            r.composer(&parent, &["create-project", "symfony/skeleton", &dir_arg])?;
            if o.kit.is_empty() {
                host.step("Adding Symfony's web app (composer require webapp)");
                // Without a terminal, the web app's recipes add Docker Compose files (N1).
                r.composer(&o.dir, &["config", "--json", "extra.symfony.docker", "false"])?;
                r.composer(&o.dir, &["require", "webapp"])?;
                host.step("Setting .env.local: database, Mail");
                let url = match &account {
                    Some((engine, port, password)) => symfony_db_url(*engine, *port, &db_name(&o.name), password),
                    None => "sqlite:///%kernel.project_dir%/var/data_%kernel.environment%.db".into(),
                };
                let mut pairs = vec![pair("DATABASE_URL", &format!("\"{url}\""))];
                if let Some(port) = o.mail {
                    pairs.push(pair("MAILER_DSN", &format!("smtp://127.0.0.1:{port}")));
                }
                edit_env(&o.dir.join(".env.local"), &pairs, EnvStyle::Dotenv)?;
            }
        }
        "codeigniter" => {
            host.step("Creating CodeIgniter 4 (composer create-project)");
            r.composer(&parent, &["create-project", "codeigniter4/appstarter", &dir_arg])?;
            host.step("Setting .env: environment and base URL");
            let template = fs::read_to_string(o.dir.join("env")).unwrap_or_default();
            let pairs = [pair("CI_ENVIRONMENT", "development"), pair("app.baseURL", &format!("'{}/'", o.url))];
            fs::write(o.dir.join(".env"), set_env(&template, &pairs, EnvStyle::Spaced)).map_err(|e| e.to_string())?;
        }
        "wordpress" => wordpress(o, r, host, &mut made)?,
        t if find(t).is_ok_and(|t| t.node.is_some()) => node_project(o, r, host, &mut made)?,
        other => return Err(format!("no template {other:?}")),
    }
    host.step("Serving the site");
    Ok(made)
}

/// A Node.js project from its framework's own create tool (N1's options), without the tool's own
/// install or Git repository; then `npm install`, so a failed install stops the job.
fn node_project(o: &Options, r: &Runner, host: &dyn Host, made: &mut Made) -> Result<(), String> {
    let app = find(o.template)?.node.ok_or("not a Node.js template")?;
    let parent = o.dir.parent().ok_or("the project folder has no parent")?.to_path_buf();
    let folder = o.dir.file_name().map(|f| f.to_string_lossy().into_owned()).ok_or("the project folder has no name")?;
    host.step("Checking Node.js");
    let v = r.node(&parent)?;
    let range = r.npm(&parent, &["view", app.framework, "engines.node"])?.trim().trim_matches('\'').to_string();
    if !range.is_empty() && !node_satisfies(v, &range) {
        return Err(format!(
            "{} needs Node.js {range}; this is {}.{}.{} (`nvm install --lts` gets a newer one)",
            app.framework, v.0, v.1, v.2
        ));
    }
    host.step(node_create_step(o.template));
    let f = folder.as_str();
    match o.template {
        "next" => r.npx(
            &parent,
            &[
                "--yes",
                "create-next-app@latest",
                f,
                "--yes",
                "--ts",
                "--eslint",
                "--tailwind",
                "--app",
                "--import-alias",
                "@/*",
                "--use-npm",
                "--skip-install",
                "--disable-git",
            ],
        )?,
        "nuxt" => r.npm(
            &parent,
            &[
                "create",
                "nuxt@latest",
                f,
                "--",
                "--template",
                "minimal",
                "--packageManager",
                "npm",
                "--no-gitInit",
                "--no-install",
                "--modules=",
            ],
        )?,
        "sveltekit" => r.npx(
            &parent,
            &[
                "--yes",
                "sv@latest",
                "create",
                f,
                "--template",
                "minimal",
                "--types",
                "ts",
                "--no-add-ons",
                "--no-install",
            ],
        )?,
        "astro" => r.npm(
            &parent,
            &[
                "create",
                "astro@latest",
                f,
                "--",
                "--template",
                "basics",
                "--yes",
                "--no-install",
                "--no-git",
                "--skip-houston",
            ],
        )?,
        "express" => {
            fs::create_dir_all(&o.dir).map_err(|e| format!("{}: {e}", o.dir.display()))?;
            let package = json!({
                "name": o.name,
                "version": "0.1.0",
                "private": true,
                "type": "module",
                "scripts": {"dev": "node --watch server.js", "start": "node server.js"},
            });
            let package = serde_json::to_string_pretty(&package).map_err(|e| e.to_string())?;
            fs::write(o.dir.join("package.json"), package + "\n").map_err(|e| e.to_string())?;
            fs::write(o.dir.join("server.js"), EXPRESS_SERVER).map_err(|e| e.to_string())?;
            String::new()
        }
        vite => {
            let template = match vite {
                "vite" => "vanilla-ts".to_string(),
                other => format!("{other}-ts"),
            };
            r.npm(
                &parent,
                &["create", "vite@latest", f, "--", "--template", &template, "--no-interactive", "--no-immediate"],
            )?
        }
    };
    if !o.dir.join("package.json").is_file() {
        return Err(format!("the create tool made no package.json in {}; see {}", o.dir.display(), r.log.display()));
    }
    host.step("Installing packages (npm install)");
    match o.template {
        "express" => r.npm(&o.dir, &["install", "express"])?,
        _ => r.npm(&o.dir, &["install"])?,
    };
    made.commands.push(("dev".into(), app.dev.into()));
    if let Some(port) = o.port {
        made.notes.push(format!("the first visit starts `{}` on port {port}; it stops when idle", app.dev));
    }
    Ok(())
}

/// The Express API template's server: Devid writes it, since Express has no create tool.
const EXPRESS_SERVER: &str = r#"import express from 'express';

const app = express();
app.use(express.json());

app.get('/', (req, res) => {
  res.json({ message: 'Hello from Express', time: new Date().toISOString() });
});

const port = Number(process.env.PORT) || 3000;
app.listen(port, '127.0.0.1', () => {
  console.log(`Listening on http://127.0.0.1:${port}`);
});
"#;

/// The template's database account, made before any tool runs, so a missing server or a taken
/// name stops the job before it downloads anything. Returns (engine, port, password).
fn account(o: &Options, host: &dyn Host, made: &mut Made) -> Result<Option<(Engine, u16, String)>, String> {
    let Some(engine) = o.db else { return Ok(None) };
    host.step("Making the database account");
    let (_, port) = host.database(engine)?;
    let name = db_name(&o.name);
    let password = host.add_account(engine, &name)?;
    made.account = Some((engine, name, password.clone()));
    Ok(Some((engine, port, password)))
}

/// The `.env` lines that send an app's mail to Mail, while Mail is on (D-049).
fn mail_pairs(o: &Options) -> Vec<(String, String)> {
    match o.mail {
        Some(port) => vec![
            pair("MAIL_MAILER", "smtp"),
            pair("MAIL_HOST", "127.0.0.1"),
            pair("MAIL_PORT", &port.to_string()),
            pair("MAIL_FROM_ADDRESS", &format!("\"hello@{}\"", o.host)),
        ],
        None => vec![],
    }
}

fn pair(key: &str, value: &str) -> (String, String) {
    (key.to_string(), value.to_string())
}

/// Doctrine's URL for a Devid account. Generated passwords are letters and digits; a chosen one
/// may hold characters a URL must escape.
pub fn symfony_db_url(engine: Engine, port: u16, name: &str, password: &str) -> String {
    let password: String = password
        .bytes()
        .map(|b| match b {
            b'A'..=b'Z' | b'a'..=b'z' | b'0'..=b'9' | b'-' | b'_' | b'.' | b'~' => (b as char).to_string(),
            _ => format!("%{b:02X}"),
        })
        .collect();
    match engine {
        Engine::Postgresql => {
            format!("postgresql://{name}:{password}@127.0.0.1:{port}/{name}?serverVersion=18&charset=utf8")
        }
        _ => format!("mysql://{name}:{password}@127.0.0.1:{port}/{name}?serverVersion=8.4&charset=utf8mb4"),
    }
}

/// WordPress through WP-CLI, on a MySQL account of Devid's (N1's findings: the zip, a checksum
/// check, and `siteurl` checked after the install).
fn wordpress(o: &Options, r: &Runner, host: &dyn Host, made: &mut Made) -> Result<(), String> {
    host.step("Getting WP-CLI");
    let paths_home = r.wp.parent().and_then(Path::parent).ok_or("WP-CLI's folder")?;
    if let Some(version) =
        install::ensure_wp_cli(&Paths { home: paths_home.to_path_buf() }).map_err(|e| format!("WP-CLI: {e}"))?
    {
        r.note(&format!("downloaded WP-CLI {version}, its SHA-512 checked"));
    }
    let Some((_, port, password)) = account(o, host, made)? else {
        return Err("WordPress needs MySQL".into());
    };
    host.step("Downloading WordPress");
    // `wp core download`'s own archive is a .tar.gz, which WP-CLI 2.12 unpacks with names cut at
    // 100 characters; the zip from the version API is whole (N1).
    let offers = install::curl_text("https://api.wordpress.org/core/version-check/1.7/")
        .map_err(|e| format!("wordpress.org's version API: {e}"))?;
    let offers: Value = serde_json::from_str(&offers).map_err(|e| format!("wordpress.org's version API: {e}"))?;
    let zip = offers["offers"][0]["packages"]["full"]
        .as_str()
        .filter(|u| u.starts_with("https://") && u.ends_with(".zip"))
        .ok_or("wordpress.org's version API offered no zip")?
        .to_string();
    fs::create_dir_all(&o.dir).map_err(|e| format!("{}: {e}", o.dir.display()))?;
    r.wp(&o.dir, &["core".into(), "download".into(), zip])?;
    host.step("Checking WordPress's files");
    r.wp(&o.dir, &["core".into(), "verify-checksums".into()])?;
    host.step("Writing wp-config.php");
    let name = db_name(&o.name);
    r.wp(
        &o.dir,
        &[
            "config".into(),
            "create".into(),
            format!("--dbname={name}"),
            format!("--dbuser={name}"),
            format!("--dbpass={password}"),
            format!("--dbhost=127.0.0.1:{port}"),
        ],
    )?;
    let admin = crate::db::password();
    let mut args: Vec<String> = vec![
        format!("--url={}", o.url),
        format!("--title={}", o.name),
        "--admin_user=admin".into(),
        format!("--admin_password={admin}"),
        format!("--admin_email=admin@{}", o.host),
        "--skip-email".into(),
    ];
    match o.multisite {
        // D-065: a network. WP-CLI adds its constants to wp-config.php; the web servers read them
        // (`sites::wp_subdir_network`), and Apache reads the .htaccess WordPress documents.
        Some(kind) => {
            host.step("Installing WordPress Multisite");
            args.splice(0..0, ["core".to_string(), "multisite-install".to_string()]);
            if kind == "subdomain" {
                args.push("--subdomains".into());
            }
            r.wp(&o.dir, &args)?;
            let rules = if kind == "subdomain" { HTACCESS_SUBDOMAIN } else { HTACCESS_SUBDIR };
            fs::write(o.dir.join(".htaccess"), rules).map_err(|e| format!(".htaccess: {e}"))?;
        }
        None => {
            host.step("Installing WordPress");
            args.splice(0..0, ["core".to_string(), "install".to_string()]);
            r.wp(&o.dir, &args)?;
        }
    }
    for option in ["siteurl", "home"] {
        let value = r.wp(&o.dir, &["option".into(), "get".into(), option.into()])?;
        if value.trim() != o.url {
            r.note(&format!("{option} was {}; setting {}", value.trim(), o.url));
            r.wp(&o.dir, &["option".into(), "update".into(), option.into(), o.url.clone()])?;
        }
    }
    made.admin = Some(("admin".into(), admin));
    made.notes.push(format!("sign in at {}/wp-admin; `wp user update admin --user_pass=…` sets a new password", o.url));
    match o.multisite {
        Some("subdomain") => made.notes.push(format!(
            "a network of sites on subdomains: `wp site create --slug=shop` adds https://shop.{}",
            o.url.trim_start_matches("https://")
        )),
        Some(_) => made
            .notes
            .push(format!("a network of sites in subfolders: `wp site create --slug=shop` adds {}/shop/", o.url)),
        None => {}
    }
    Ok(())
}

/// WordPress's own .htaccess for a network (from its Network Setup), for Apache.
const HTACCESS_SUBDIR: &str = "# BEGIN WordPress Multisite (written by Devid, from WordPress's Network Setup)
RewriteEngine On
RewriteRule .* - [E=HTTP_AUTHORIZATION:%{HTTP:Authorization}]
RewriteBase /
RewriteRule ^index\\.php$ - [L]

# add a trailing slash to /wp-admin
RewriteRule ^([_0-9a-zA-Z-]+/)?wp-admin$ $1wp-admin/ [R=301,L]

RewriteCond %{REQUEST_FILENAME} -f [OR]
RewriteCond %{REQUEST_FILENAME} -d
RewriteRule ^ - [L]
RewriteRule ^([_0-9a-zA-Z-]+/)?(wp-(content|admin|includes).*) $2 [L]
RewriteRule ^([_0-9a-zA-Z-]+/)?(.*\\.php)$ $2 [L]
RewriteRule . index.php [L]
# END WordPress Multisite
";

const HTACCESS_SUBDOMAIN: &str = "# BEGIN WordPress Multisite (written by Devid, from WordPress's Network Setup)
RewriteEngine On
RewriteRule .* - [E=HTTP_AUTHORIZATION:%{HTTP:Authorization}]
RewriteBase /
RewriteRule ^index\\.php$ - [L]

# add a trailing slash to /wp-admin
RewriteRule ^wp-admin$ wp-admin/ [R=301,L]

RewriteCond %{REQUEST_FILENAME} -f [OR]
RewriteCond %{REQUEST_FILENAME} -d
RewriteRule ^ - [L]
RewriteRule ^(wp-(content|admin|includes).*) $1 [L]
RewriteRule ^(.*\\.php)$ $1 [L]
RewriteRule . index.php [L]
# END WordPress Multisite
";

/// What `devid new --list` shows: each template, its kits and databases, and why it cannot run here.
pub fn templates_json(php_installed: bool, node: Option<(u32, u32, u32)>) -> Value {
    let list: Vec<Value> = TEMPLATES
        .iter()
        .map(|t| {
            let kits: Vec<&str> = t.kits.iter().copied().filter(|k| !k.is_empty()).collect();
            let mut limits = Vec::new();
            if t.node.is_some() && node.is_none() {
                limits.push("needs Node.js; none found (nvm's default, or node on your login shell's PATH)".into());
            }
            if t.key != "static" && t.node.is_none() && !php_installed {
                limits.push("no PHP installed; `devid php install 8.5`".to_string());
            }
            if t.key == "laravel" {
                match node {
                    Some(v) if kit_node_ok(v) => {}
                    Some(v) => limits
                        .push(format!("the starter kits need Node.js {KIT_NODE}; this is {}.{}.{}", v.0, v.1, v.2)),
                    None => limits.push(format!("the starter kits need Node.js {KIT_NODE}; none found")),
                }
            }
            json!({"template": t.key, "label": t.label, "kits": kits, "dbs": t.dbs, "limits": limits})
        })
        .collect();
    json!(list)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn names_work_as_hosts_and_database_names() {
        for ok in ["shop", "my-shop", "a1", &"a".repeat(32)] {
            assert!(check_name(ok).is_ok(), "{ok}");
        }
        for bad in ["", "1shop", "-shop", "shop-", "Shop", "my_shop", "my.shop", &"a".repeat(33)] {
            assert!(check_name(bad).is_err(), "{bad}");
        }
        assert_eq!(db_name("my-shop"), "my_shop");
    }

    #[test]
    fn kits_and_databases_per_template() {
        let laravel = find("laravel").unwrap();
        assert_eq!(choose(laravel, None, None).unwrap(), (String::new(), None, true));
        assert_eq!(
            choose(laravel, Some("react"), Some("mysql")).unwrap(),
            ("react".into(), Some(Engine::Mysql), false)
        );
        assert_eq!(choose(laravel, None, Some("pgsql")).unwrap().1, Some(Engine::Postgresql));
        assert!(choose(laravel, Some("angular"), None).unwrap_err().contains("react, vue, svelte, livewire"));
        assert!(choose(laravel, None, Some("mongodb")).is_err());
        let symfony = find("symfony").unwrap();
        assert_eq!(choose(symfony, None, None).unwrap(), (String::new(), None, true));
        assert_eq!(choose(symfony, Some("skeleton"), None).unwrap(), ("skeleton".into(), None, false));
        assert!(choose(symfony, Some("skeleton"), Some("mysql")).is_err());
        let wordpress = find("wordpress").unwrap();
        assert_eq!(choose(wordpress, None, None).unwrap().1, Some(Engine::Mysql));
        assert!(choose(wordpress, None, Some("pgsql")).is_err());
        assert!(choose(find("php").unwrap(), None, Some("mysql")).is_err());
        assert!(choose(find("static").unwrap(), Some("react"), None).is_err());
        assert!(find("next").unwrap().node.is_some());
        assert!(choose(find("astro").unwrap(), None, Some("mysql")).is_err());
        assert!(find("rails").unwrap_err().contains("no template"));
    }

    #[test]
    fn node_for_the_starter_kits() {
        assert_eq!(parse_node_version("v24.22.3\n"), Some((24, 22, 3)));
        assert_eq!(parse_node_version("nope"), None);
        for ok in [(22, 18, 0), (22, 20, 1), (24, 11, 0), (26, 0, 0), (27, 1, 0)] {
            assert!(kit_node_ok(ok), "{ok:?}");
        }
        for bad in [(20, 19, 0), (22, 17, 9), (23, 5, 0), (24, 10, 0), (25, 0, 0)] {
            assert!(!kit_node_ok(bad), "{bad:?}");
        }
    }

    #[test]
    fn env_lines_replace_uncomment_or_append() {
        let laravel = "APP_URL=http://localhost:8000\nDB_CONNECTION=sqlite\n# DB_HOST=127.0.0.1\n# DB_PORT=3306\n\
                       MAIL_MAILER=log\n";
        let out = set_env(
            laravel,
            &[
                pair("APP_URL", "https://shop.test"),
                pair("DB_CONNECTION", "mysql"),
                pair("DB_HOST", "127.0.0.1"),
                pair("DB_PASSWORD", "secret"),
            ],
            EnvStyle::Dotenv,
        );
        assert_eq!(
            out,
            "APP_URL=https://shop.test\nDB_CONNECTION=mysql\nDB_HOST=127.0.0.1\n# DB_PORT=3306\nMAIL_MAILER=log\n\
             DB_PASSWORD=secret\n"
        );
        let ci = "#--------\n# ENVIRONMENT\n#--------\n\n# CI_ENVIRONMENT = production\n\n# app.baseURL = ''\n";
        let out = set_env(
            ci,
            &[pair("CI_ENVIRONMENT", "development"), pair("app.baseURL", "'https://shop.test/'")],
            EnvStyle::Spaced,
        );
        assert!(out.contains("\nCI_ENVIRONMENT = development\n"), "{out}");
        assert!(out.contains("\napp.baseURL = 'https://shop.test/'\n"), "{out}");
        assert!(out.starts_with("#--------\n# ENVIRONMENT\n"), "comments without `=` stay: {out}");
        // A real line wins over a commented one, and an empty file gets the line.
        assert_eq!(set_env("# A=1\nA=2\n", &[pair("A", "3")], EnvStyle::Dotenv), "# A=1\nA=3\n");
        assert_eq!(set_env("", &[pair("A", "1")], EnvStyle::Dotenv), "A=1\n");
    }

    #[test]
    fn folders_must_be_new_or_empty() {
        let root = std::env::temp_dir().join(format!("devid-new-dir-{}", std::process::id()));
        fs::create_dir_all(root.join("empty")).unwrap();
        fs::create_dir_all(root.join("full")).unwrap();
        fs::write(root.join("full/x"), "").unwrap();
        fs::write(root.join("file"), "").unwrap();
        assert!(check_dir(&root.join("new")).is_ok());
        assert!(check_dir(&root.join("empty")).is_ok());
        assert!(check_dir(&root.join("full")).unwrap_err().contains("not empty"));
        assert!(check_dir(&root.join("file")).unwrap_err().contains("not a folder"));
        assert!(check_dir(&root.join("missing/new")).unwrap_err().contains("does not exist"));
        assert!(check_dir(Path::new("relative")).is_err());
        fs::remove_dir_all(&root).unwrap();
    }

    fn options(template: &'static str, kit: &str, db: Option<Engine>) -> Options {
        Options {
            template,
            name: "shop".into(),
            dir: "/tmp/sites/shop".into(),
            php: "8.5".into(),
            kit: kit.into(),
            db,
            sqlite: db.is_none(),
            host: "shop.test".into(),
            url: "https://shop.test".into(),
            mail: Some(2525),
            port: None,
            multisite: None,
        }
    }

    #[test]
    fn multisite_is_for_wordpress() {
        let wordpress = find("wordpress").unwrap();
        assert_eq!(multisite(wordpress, None).unwrap(), None);
        assert_eq!(multisite(wordpress, Some("subdir")).unwrap(), Some("subdir"));
        assert_eq!(multisite(wordpress, Some("subdomain")).unwrap(), Some("subdomain"));
        assert!(multisite(wordpress, Some("both")).unwrap_err().contains("subdomain or subdir"));
        assert!(multisite(find("laravel").unwrap(), Some("subdir")).unwrap_err().contains("is for WordPress"));
        let network = Options { multisite: Some("subdir"), ..options("wordpress", "", Some(Engine::Mysql)) };
        assert!(steps(&network).contains(&"Installing WordPress Multisite"));
        assert!(HTACCESS_SUBDIR.contains("RewriteRule ^([_0-9a-zA-Z-]+/)?(wp-(content|admin|includes).*) $2 [L]"));
        assert!(HTACCESS_SUBDOMAIN.contains("RewriteRule ^(wp-(content|admin|includes).*) $1 [L]"));
    }

    #[test]
    fn steps_follow_the_choices() {
        assert_eq!(steps(&options("php", "", None)), ["Writing the first page", "Serving the site"]);
        let plain = steps(&options("laravel", "", None));
        assert!(!plain.iter().any(|s| s.contains("Node") || s.contains("account") || s.contains("Migrating")));
        let kit = steps(&options("laravel", "react", Some(Engine::Mysql)));
        assert_eq!(kit[0], "Checking Node.js");
        assert_eq!(kit[1], "Making the database account");
        assert!(kit.contains(&"Migrating the database"));
        assert!(!steps(&options("symfony", "skeleton", None)).iter().any(|s| s.contains("webapp")));
        assert_eq!(steps(&options("wordpress", "", Some(Engine::Mysql))).len(), 7);
        assert_eq!(
            steps(&options("next", "", None)),
            [
                "Checking Node.js",
                "Creating the Next.js app (create-next-app)",
                "Installing packages (npm install)",
                "Serving the site"
            ]
        );
        assert_eq!(steps(&options("react", "", None))[1], "Creating the Vite project (npm create vite)");
    }

    #[test]
    fn node_ranges_as_frameworks_write_them() {
        let vite = "^20.19.0 || >=22.12.0";
        for (v, ok) in
            [((20, 19, 0), true), ((20, 18, 9), false), ((21, 0, 0), false), ((22, 12, 0), true), ((24, 1, 0), true)]
        {
            assert_eq!(node_satisfies(v, vite), ok, "{v:?}");
        }
        let nuxt = "^22.22.3 || ^24.15.0 || >=26.0.0";
        for (v, ok) in
            [((22, 22, 3), true), ((22, 22, 2), false), ((23, 0, 0), false), ((24, 22, 3), true), ((26, 1, 0), true)]
        {
            assert_eq!(node_satisfies(v, nuxt), ok, "{v:?}");
        }
        assert!(node_satisfies((22, 17, 0), ">=22.17") && !node_satisfies((22, 16, 9), ">=22.17"));
        assert!(node_satisfies((18, 0, 0), ">= 18") && !node_satisfies((16, 20, 0), ">= 18"));
        assert!(node_satisfies((20, 9, 0), ">=20.9.0"));
        assert!(node_satisfies((18, 3, 0), "18") && !node_satisfies((19, 0, 0), "18"));
        assert!(node_satisfies((18, 3, 0), "~18.3") && !node_satisfies((18, 4, 0), "~18.3"));
        assert!(node_satisfies((20, 0, 0), ">=18 <21") && !node_satisfies((21, 0, 0), ">=18 <21"));
    }

    #[test]
    fn progress_marks_steps_and_the_failed_one() {
        let o = options("codeigniter", "", None);
        let mut job = Job::new(&o, "/tmp/new-shop.log".into());
        job.start("Setting .env: environment and base URL");
        let v = job.json();
        assert_eq!(v["steps"][0]["state"], "done");
        assert_eq!(v["steps"][1]["state"], "running");
        assert_eq!(v["steps"][2]["state"], "pending");
        job.finish(&Err("boom".into()));
        let v = job.json();
        assert_eq!(v["steps"][1]["state"], "failed");
        assert_eq!(v["error"], "boom");
        assert_eq!(v["done"], true);
    }

    #[test]
    fn secrets_stay_out_of_the_log_and_urls_are_escaped() {
        assert_eq!(hide_secret("--dbpass=abc"), "--dbpass=…");
        assert_eq!(hide_secret("--admin_password=abc"), "--admin_password=…");
        assert_eq!(hide_secret("--dbname=shop"), "--dbname=shop");
        assert_eq!(
            symfony_db_url(Engine::Mysql, 3307, "shop", "a@b%c"),
            "mysql://shop:a%40b%25c@127.0.0.1:3307/shop?serverVersion=8.4&charset=utf8mb4"
        );
        assert!(
            symfony_db_url(Engine::Postgresql, 5433, "shop", "x").starts_with("postgresql://shop:x@127.0.0.1:5433/")
        );
    }

    #[test]
    fn the_list_says_what_is_missing() {
        let v = templates_json(true, Some((20, 19, 0)));
        let get = |k: &str| v.as_array().unwrap().iter().find(|t| t["template"] == k).unwrap().clone();
        assert!(get("laravel")["limits"][0].as_str().unwrap().contains("22.18+"));
        assert_eq!(get("php")["limits"], json!([]));
        assert_eq!(get("wordpress")["dbs"], json!(["mysql"]));
        let none = templates_json(false, None);
        assert!(none.as_array().unwrap()[0]["limits"][0].as_str().unwrap().contains("no PHP"));
        assert_eq!(none.as_array().unwrap().iter().find(|t| t["template"] == "static").unwrap()["limits"], json!([]));
    }
}
