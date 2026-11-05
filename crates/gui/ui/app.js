// The Sites page, filled from the daemon. The app only renders daemon state (D-012).
'use strict';

const invoke = window.__TAURI__.core.invoke;
const $ = (id) => document.getElementById(id);
const POLL_MS = 2000; // as the TUI
let siteCount = null;

function row(template) {
  return $(template).content.firstElementChild.cloneNode(true);
}

function fill(el, key, text) {
  el.querySelector('[data-' + key + ']').textContent = text;
}

function capital(s) {
  return s.charAt(0).toUpperCase() + s.slice(1);
}

function showStatus(s) {
  const failed = s.php.filter((p) => p.status === 'failed').length + (s.server_status === 'failed' ? 1 : 0);
  const running = s.server_status === 'running';
  $('error').hidden = true;
  $('status-dot').className = 'dv-dot ' + (failed ? 'bad' : running ? 'ok' : 'warn');
  $('status-server').textContent = s.server;
  $('status-line').textContent =
    (failed ? failed + (failed === 1 ? ' service failed' : ' services failed') : capital(s.server_status)) +
    ' · PHP ' + s.default_php;
  $('nav-sites').textContent = s.sites;
  $('nav-php').textContent = s.default_php;
}

function showError(message) {
  $('error').hidden = false;
  $('error-msg').textContent = message; // the daemon client's own wording (D-026)
  $('status-dot').className = 'dv-dot bad';
  $('status-server').textContent = 'Devid';
  $('status-line').textContent = 'Not running';
}

function showSites(r) {
  const parked = r.parked.map((folder) => {
    const n = r.sites.filter((s) => !s.linked && s.path.startsWith(folder + '/')).length;
    const el = row('parked-row');
    fill(el, 'path', folder);
    fill(el, 'count', n ? n + ' site' + (n > 1 ? 's' : '') + ' served from here' : 'No sites yet');
    return el;
  });
  $('parked').replaceChildren(...parked);
  $('parked-count').textContent = '(' + r.parked.length + ')';

  const sites = r.sites.map((s) => {
    const el = row('site-row');
    fill(el, 'initial', s.name.charAt(0).toUpperCase());
    fill(el, 'host', new URL(s.url).hostname);
    fill(el, 'path', s.path);
    fill(el, 'kind', s.linked ? 'Linked' : 'Parked');
    fill(el, 'php', s.php);
    el.querySelector('[data-isolated]').hidden = !s.isolated;
    if (!s.php_installed) {
      const missing = el.querySelector('[data-missing]');
      missing.hidden = false;
      missing.textContent = 'PHP ' + s.php + ' not installed'; // the TUI's wording
      el.querySelector('[data-php]').classList.add('off');
    }
    return el;
  });
  if (!sites.length) {
    const empty = document.createElement('div');
    empty.className = 'dv-empty dv-sub';
    empty.textContent = 'No sites yet. Park a folder or link one with the CLI.';
    sites.push(empty);
  }
  $('sites').replaceChildren(...sites);
  $('sites-count').textContent = '(' + r.sites.length + ')';
}

async function refresh() {
  try {
    const s = await invoke('status');
    showStatus(s);
    if (s.sites !== siteCount) {
      showSites(await invoke('sites'));
      siteCount = s.sites;
    }
  } catch (e) {
    siteCount = null;
    showError(String(e));
  }
}

// Poll only while the window is visible; a closed window is destroyed with its web view.
let timer = null;
function poll() {
  clearInterval(timer);
  timer = document.hidden ? null : setInterval(refresh, POLL_MS);
  if (!document.hidden) refresh();
}
document.addEventListener('visibilitychange', poll);
poll();
