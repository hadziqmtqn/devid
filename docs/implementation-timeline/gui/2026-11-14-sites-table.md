# The Sites page as a table, a quick menu on the status card, and a Link a site dialog

> Date: 2026-11-14 · Branch: `gui/sites-table`

## Problem

- Each site row put its chips, port, running commands, PHP pick and two icon buttons in one line, so nothing lined up from row to row.
- Linking a site served the folder as soon as it was picked. Devid's guess for its type, dev server port and PHP version could only be changed afterwards, in the site's detail.

## Change

The design is the "Sites", "Sites · Link a site" and "Quick menu" boards on the v0.12.x page of the Devid Redesign canvas.

### Daemon

`link_preview { name, path }` runs `link`'s logic on a copy of the config and saves nothing. It returns:
- the name, type, mode and PHP version;
- the dev server port after D-042's spreading;
- `taken`, the path of a site with that name.

`link` and `link_preview` share the name rule (`link_name`).

### App, Sites page

- **A table** with Site, Serves and Processes columns:
  - **Site:** the host, its type, Isolated, "PHP x not installed" and Shared, then the path and "· Linked".
  - **Serves:** the PHP version picker, "Proxy :port" or "Static".
  - **Processes:** "N running" or "Not running", and the local port ("on :8000", "· busy" in orange while another program holds it).
- **The host** opens the site in the browser. **›** opens the detail.
- **⋯ menu:** Open in browser, Copy URL, Open folder in Terminal, Details, then Remove site… or Unlink….
- **PHP picker:** ✓ marks the current version and "default" the default one. Picking the default follows the default again (`unisolate`); another version isolates the site.
- **Served / Removed** is always on the Sites line. Removed sites are rows of the same table, with Open folder in Terminal and Serve again. The parked folder's line says how many sites inside it are removed.
- **Unchanged:** the "Devid is stopped" banner, and Static sites.

### App, quick menu

⋯ on the status card opens a menu beside the sidebar:
- the server and PHP status;
- every site, which opens it, with how it is served;
- Stop all and Restart, or Start all;
- the default PHP as buttons;
- Settings…;
- Quit Devid.

### App, Link a site

1. Choose… picks a folder.
2. The dialog shows the domain and the type.
3. It asks for the dev server port of a proxy site, or the PHP version of a PHP site.
4. A name already in use is refused before anything is saved.
5. Link site sends `link`, then `app_set` for a changed port, or `isolate` for a version other than the default.

### App, Rust

- `quit` exits as the menu-bar menu's Quit does.
- Terminal may open a removed site's folder.

## Verification

- `cargo fmt --check`, `cargo clippy --all-targets -- -D warnings` (also `-p devid-gui`), `cargo test` and `cargo test -p devid-gui` pass.
- **Integration test (`a_new_project_gets_a_free_dev_server_port`):**
  - a preview of a Next.js folder outside the parked one gets `c-new`, `next`, `proxy` and a free port;
  - nothing is saved;
  - a name in use reports its path in `taken`.
- **The real `app.js` and `app.css` in headless Brave**, with a fake `invoke` that answers with the owner's live `status` and `sites` replies. Checked in light and dark, at 1080 and 760 px wide:
  - the table, the PHP picker, the row menu, the Removed view, the quick menu (running and stopped) and the dialog (empty, PHP, proxy, name taken);
  - the requests:
    - `link` then `app_set upstream 3005` for a proxy site with a changed port;
    - `link` then `isolate 8.5` for a PHP site;
    - `unisolate` when the default is picked on an isolated site, `isolate` for another version;
    - `php_use` from the quick menu, and `quit`;
  - a bad port and a name in use keep the dialog open with the reason.
- **Not run by hand:** the page in Devid.app, the folder picker, and Quit from the window.
