// The Devid window (Phase 7, D-026): renders daemon state and sends daemon requests (D-012).
// Messages use the CLI's and TUI's wording, in sentence case. The DOM is built with h(), which
// only ever sets text, never HTML from data.
'use strict';

const { invoke } = window.__TAURI__.core;
const daemon = (req) => invoke('daemon', { req });
const SUPPORTED_PHP = ['8.3', '8.4', '8.5']; // php::SUPPORTED (D-009)
const SERVERS = {
  nginx: { label: 'nginx', hint: 'Fast and familiar if you come from Valet or Forge.', recommended: true },
  caddy: { label: 'Caddy', hint: 'Short, readable config.' },
  apache: { label: 'Apache', hint: 'The httpd that ships with macOS. For apps that rely on .htaccess.' },
};
// App types (D-035), in sites::PRESETS order: [key, label].
const PRESETS = [['laravel', 'Laravel'], ['ci4', 'CodeIgniter 4'], ['ci3', 'CodeIgniter 3'], ['symfony', 'Symfony'],
  ['wordpress', 'WordPress'], ['php', 'PHP'], ['next', 'Next.js'], ['nuxt', 'Nuxt'], ['node', 'Node.js'], ['vite', 'Vite'], ['static', 'Static']];
const MODES = { php: 'PHP', static: 'Static', proxy: 'Proxy' };
const EXT_INFO = {
  xdebug: 'Debugger; turn it on only while debugging.',
  pcov: 'Code coverage for PHPUnit and Pest.',
  apcu: 'In-memory cache',
  igbinary: 'Serializer for Redis and cache',
  imagick: 'Image processing with ImageMagick',
  ldap: 'LDAP client',
  memcached: 'Memcached client',
  mongodb: 'MongoDB driver',
  msgpack: 'MessagePack serializer',
  swoole: 'Async server for Laravel Octane',
};
const ICONS = {
  sites: '<svg viewBox="0 0 16 16"><circle cx="8" cy="8" r="6.2"/><ellipse cx="8" cy="8" rx="2.7" ry="6.2"/><path d="M2 8h12"/></svg>',
  php: '<svg viewBox="0 0 16 16"><path d="M5 4 1.5 8 5 12M11 4l3.5 4L11 12M9.3 2.8 6.7 13.2"/></svg>',
  server: '<svg viewBox="0 0 16 16"><rect x="2" y="2.5" width="12" height="4.5" rx="1.3"/><rect x="2" y="9" width="12" height="4.5" rx="1.3"/><circle class="f" cx="4.8" cy="4.75" r=".9"/><circle class="f" cx="4.8" cy="11.25" r=".9"/></svg>',
  logs: '<svg viewBox="0 0 16 16"><rect x="3" y="1.8" width="10" height="12.4" rx="1.6"/><path d="M5.5 5.5h5M5.5 8h5M5.5 10.5h3"/></svg>',
  settings: '<svg viewBox="0 0 16 16"><path d="M2.5 4.5h7M12.5 4.5h1M2.5 11.5h1M6.5 11.5h7"/><circle cx="11" cy="4.5" r="1.6"/><circle cx="5" cy="11.5" r="1.6"/></svg>',
  folder: '<svg viewBox="0 0 16 16"><path d="M1.8 4.2a1.2 1.2 0 0 1 1.2-1.2h3.3l1.5 1.5H13a1.2 1.2 0 0 1 1.2 1.2v6.6A1.2 1.2 0 0 1 13 13.5H3a1.2 1.2 0 0 1-1.2-1.2z"/></svg>',
  more: '<svg viewBox="0 0 16 16"><circle class="f" cx="3.5" cy="8" r="1.3"/><circle class="f" cx="8" cy="8" r="1.3"/><circle class="f" cx="12.5" cy="8" r="1.3"/></svg>',
  open: '<svg viewBox="0 0 16 16"><path d="M9.5 2.5h4v4M13.5 2.5 7.5 8.5M11.5 9.5v3a1 1 0 0 1-1 1h-7a1 1 0 0 1-1-1v-7a1 1 0 0 1 1-1h3"/></svg>',
  plus: '<svg viewBox="0 0 16 16"><path d="M8 3v10M3 8h10"/></svg>',
  chevron: '<svg viewBox="0 0 16 16"><path d="m4 6 4 4 4-4"/></svg>',
  back: '<svg viewBox="0 0 16 16"><path d="M10 3.5 5.5 8l4.5 4.5"/></svg>',
  next: '<svg viewBox="0 0 16 16"><path d="m6 3.5 4.5 4.5L6 12.5"/></svg>',
  play: '<svg viewBox="0 0 16 16"><path class="f" d="M5 3.2v9.6L12.8 8z"/></svg>',
  stop: '<svg viewBox="0 0 16 16"><rect class="f" x="4" y="4" width="8" height="8" rx="1.2"/></svg>',
  search: '<svg viewBox="0 0 16 16"><circle cx="7" cy="7" r="5"/><path d="m11 11 3.5 3.5"/></svg>',
};

const S = {
  page: 'sites',
  status: null,
  error: null,
  data: {},
  keys: {},
  busy: '',
  // The open log; `tail` is its last line while it has no newline yet. `seen` and `errors` feed the
  // sources' badges: new error lines in the logs not on screen, since the Logs page first saw them.
  log: { name: null, offset: null, lines: [], tail: '', follow: true, newBelow: 0, tags: {},
    level: 'all', query: '', srcQuery: '', open: {}, seen: {}, errors: {} },
  updates: null, // {text, changes}
  devid: null, // {text, available}
  upd: { dv: null, rt: null }, // an update of Devid or of PHP and the web server: running, or how it ended
  doctor: null, // {checks} or {error} or {busy}
  ext: null, // the open Extensions dialog
  site: null, // the site whose detail view is open (Phase 8 A5)
  logPanel: null, // the command whose log shows under its site's commands
};
const $ = (id) => document.getElementById(id);

// ---------- DOM helpers ----------

function h(tag, attrs, ...kids) {
  const [name, ...classes] = tag.split('.');
  const el = document.createElement(name || 'div');
  if (classes.length) el.className = classes.join(' ');
  for (const [k, v] of Object.entries(attrs || {})) {
    if (v == null || v === false) continue;
    if (k.startsWith('on')) el.addEventListener(k.slice(2), v);
    else if (k === 'checked' || k === 'value' || k === 'disabled') el[k] = v;
    else el.setAttribute(k, v === true ? '' : v);
  }
  for (const kid of kids.flat(Infinity)) if (kid != null && kid !== false) el.append(kid);
  return el;
}

function icon(name) {
  const t = document.createElement('template');
  t.innerHTML = ICONS[name]; // constant markup, never data
  const svg = t.content.firstElementChild;
  svg.setAttribute('class', 'dv-ico');
  svg.setAttribute('aria-hidden', 'true');
  return svg;
}

const btn = (label, onclick, cls = '', extra = {}) =>
  h('button.dv-btn' + (cls ? '.' + cls : ''), { type: 'button', onclick, disabled: !!S.busy, ...extra }, label);
/** A button that shows the running action's label while it is the one running (act's `where`). */
const busyBtn = (key, label, onclick, cls = '') => btn(S.busyAt === key ? S.busy : label, onclick, cls);
const capital = (s) => s.charAt(0).toUpperCase() + s.slice(1);
const plural = (n, word) => n + ' ' + word + (n === 1 ? '' : 's');
const card = (...rows) => h('div.dv-card', {}, rows);
const row = (...kids) => h('div.dv-row', {}, kids);
const text = (title, sub) => h('div.dv-grow', {}, h('div', {}, title), sub ? h('div.dv-sub', {}, sub) : null);

const portsText = (st) => st.standard_ports
  ? 'HTTP 80 → ' + st.http_port + ' · HTTPS 443 → ' + st.https_port
  : 'HTTP ' + st.http_port + ' · HTTPS ' + st.https_port;

/** `human_size` in crates/cli/src/tui/app.rs. */
function humanSize(bytes) {
  return bytes >= 1048576 ? (bytes / 1048576).toFixed(1) + ' MB' : Math.ceil(bytes / 1024) + ' KB';
}

function andList(items) {
  return items.length > 1 ? items.slice(0, -1).join(', ') + ' and ' + items[items.length - 1] : items.join('');
}

// ---------- toasts, menus, dialogs ----------

function toast(message, bad = false, sub = '') {
  const t = h('div.dv-toast' + (bad ? '.bad' : ''), { role: bad ? 'alert' : 'status' }, h('div', {}, message), sub ? h('div.dv-sub', {}, sub) : null);
  $('toasts').append(t);
  setTimeout(() => t.remove(), bad ? 9000 : 4500);
}

let menuOpen = null;
/** `side`: an element to open beside, bottoms aligned (the status card), instead of below the anchor. */
function menu(anchor, entries, list = false, side = null) {
  closeMenu();
  const pick = (e) => ({ type: 'button', disabled: !!S.busy, onclick: () => { closeMenu(); e.run(); } });
  const m = h('div.dv-menu' + (list ? '.is-list' : ''), { role: 'menu' },
    entries.filter(Boolean).map((e) =>
      e === '-' ? h('hr.dv-sep') :
      e.header ? h('div.dv-menu-h', {}, e.dot ? h('span.dv-dot.' + e.dot, { 'aria-hidden': 'true' }) : null, e.header) :
      e.row ? h('div.dv-menu-row', { role: 'group', 'aria-label': e.label }, e.row.map((x) =>
        h('button.dv-btn' + (x.checked ? '.primary' : ''), { ...pick(x), role: 'menuitemradio', 'aria-checked': String(x.checked) }, x.label))) :
      h('button.dv-mi' + (e.danger ? '.danger' : ''), { ...pick(e), role: 'checked' in e ? 'menuitemradio' : 'menuitem',
        'aria-checked': 'checked' in e ? String(e.checked) : null },
      'checked' in e ? h('span.dv-mark', { 'aria-hidden': 'true' }, e.checked ? '✓' : '') : null,
      h('span.dv-ellipsis', {}, e.label), e.kbd ? h('span.dv-kbd', {}, e.kbd) : null)));
  document.body.append(m);
  const r = (side || anchor).getBoundingClientRect();
  const top = side ? r.bottom - m.offsetHeight : r.bottom + 4 + m.offsetHeight > innerHeight ? r.top - 4 - m.offsetHeight : r.bottom + 4;
  m.style.top = Math.max(8, top) + 'px';
  m.style.left = Math.max(8, Math.min(side ? r.right + 8 : r.right - m.offsetWidth, innerWidth - m.offsetWidth - 8)) + 'px';
  anchor.setAttribute('aria-expanded', 'true');
  menuOpen = { m, anchor };
  ((list && m.querySelector('[aria-checked="true"]')) || m.querySelector('button'))?.focus();
}

function closeMenu() {
  if (!menuOpen) return;
  menuOpen.anchor.setAttribute('aria-expanded', 'false');
  menuOpen.m.remove();
  menuOpen = null;
  if (S.dirty) renderPage();
}

/** The prototype's select field; its list is the same menu as the ⋯ menus, not a native pop-up. */
function dropdown(label, value, options, pick, shown) {
  return h('button.dv-select', { type: 'button', 'aria-label': label, 'aria-haspopup': 'menu', 'aria-expanded': 'false', disabled: !!S.busy,
    onclick: (e) => menu(e.currentTarget, options.map((o) => ({ label: o.label, kbd: o.kbd, checked: o.value === value,
      run: () => { if (o.value !== value) pick(o.value); } })), true) },
  h('span', {}, shown || (options.find((o) => o.value === value) || { label: value }).label), icon('chevron'));
}

const moreBtn = (label, entries) =>
  h('button.dv-btn.ghost.icon', { type: 'button', 'aria-label': label, 'aria-haspopup': 'menu', 'aria-expanded': 'false',
    onclick: (e) => menu(e.currentTarget, entries()) }, icon('more'));

function showDialog(node, onEscape) {
  const o = $('overlay');
  const scroll = o.firstElementChild?.scrollTop || 0; // a redraw of the same dialog keeps its place
  const same = o.firstElementChild?.getAttribute('aria-labelledby') === node.getAttribute('aria-labelledby');
  const focused = same && document.activeElement?.getAttribute('aria-label');
  o.replaceChildren(node);
  o.hidden = false;
  o.onkeydown = (e) => { if (e.key === 'Escape') onEscape(); };
  node.scrollTop = same ? scroll : 0;
  const again = focused && node.querySelector('[aria-label="' + CSS.escape(focused) + '"]');
  (again || node.querySelector('[data-focus]') || node.querySelector('button'))?.focus({ preventScroll: true });
}

function closeDialog() {
  $('overlay').hidden = true;
  $('overlay').replaceChildren();
  S.ext = null;
  if (S.dirty) renderPage();
}

/** A confirmation: a short title, one sentence on the effect, Cancel and the action (D-026). */
function ask(title, message, action, destructive = false) {
  return new Promise((resolve) => {
    const done = (yes) => { closeDialog(); resolve(yes); };
    showDialog(h('div.dv-dialog', { role: 'alertdialog', 'aria-modal': 'true', 'aria-labelledby': 'ask-t', 'aria-describedby': 'ask-m' },
      h('h2', { id: 'ask-t' }, title),
      h('p.dv-sub', { id: 'ask-m' }, message),
      h('div.dv-dialog-btns', {},
        h('button.dv-btn', { type: 'button', onclick: () => done(false), 'data-focus': destructive }, 'Cancel'),
        h('button.dv-btn.' + (destructive ? 'danger' : 'primary'), { type: 'button', onclick: () => done(true), 'data-focus': !destructive }, action))),
    () => done(false));
  });
}

/**
 * Runs one action at a time; a toast shows the result. `where` shows `label` meanwhile:
 * 'card' in the sidebar's status card (actions on running services only), a button's key on
 * that button (busyBtn), or nowhere for quick actions.
 */
async function act(label, run, done, where = '') {
  if (S.busy) return;
  S.busy = label;
  S.busyAt = where;
  renderChrome();
  renderPage();
  try {
    const reply = await run();
    const message = typeof done === 'function' ? done(reply) : done;
    if (message) toast(message);
  } catch (e) {
    toast(String(e), true);
  } finally {
    S.busy = '';
    S.busyAt = '';
    await refresh(true);
  }
}

// ---------- shared actions ----------

const restart = (service) => act('Restarting ' + service + '…', () => daemon({ cmd: 'restart', service }), 'Restarted ' + service, 'card');
const startAll = () => act('Starting…', () => daemon({ cmd: 'start_all' }), 'Started all services', 'card');
async function stopAll() {
  if (await ask('Stop all services?', 'The web server and every php-fpm stop. Sites are down until you start them.', 'Stop all'))
    act('Stopping…', () => daemon({ cmd: 'stop_all' }), 'Stopped all services', 'card');
}
const openTarget = (target) => act('Opening…', () => invoke('open', { target }), (msg) => msg);

// ---------- pages ----------

const PAGES = {
  sites: { title: 'Sites', heading: () => openedSite() && siteHost(openedSite()),
    sub: () => openedSite() ? openedSite().path + ' · ' + openedSite().preset_label
      : 'Every site is served at https://name.' + (S.status?.tld || 'test') + ' with a local certificate.',
    load: () => daemon({ cmd: 'sites' }), render: (d) => (openedSite() ? siteDetail(openedSite()) : sitesPage(d)), tools: () => S.site ? [] : [
      h('button.dv-btn.ghost.icon', { type: 'button', 'aria-label': 'Link a site', title: 'Link a site', onclick: linkSite, disabled: !!S.busy }, icon('plus'))] },
  php: { title: 'PHP', sub: () => 'PHP 8.3, 8.4 and 8.5 side by side; pick one per site.',
    load: () => daemon({ cmd: 'php_list' }), render: phpPage },
  server: { title: 'Web Server', sub: () => 'Status, engine choice and configuration.', load: () => daemon({ cmd: 'server_list' }), render: serverPage },
  logs: { title: 'Logs', sub: () => 'Log files in ' + (S.status?.home || '~/.devid') + '/log.', load: loadLogs, render: logsPage },
  settings: { title: 'Settings', sub: () => 'Preferences for Devid on this Mac.', load: async () => ({}), render: settingsPage },
};

/** The prototype's "Link a site": a folder, then what Devid detects there, before anything is saved. */
function linkSite() {
  const st = S.status;
  const f = { path: '', p: null, port: '', php: st.default_php, error: '', busy: false };
  const choose = async () => {
    let path;
    try { path = await invoke('choose_folder', { purpose: 'link' }); } catch (e) { f.error = String(e); }
    if (path) {
      try {
        const p = await daemon({ cmd: 'link_preview', name: null, path });
        Object.assign(f, { path, p, port: String(p.upstream), php: p.php, error: p.taken ? p.host + ' is already in use by ' + p.taken + '.' : '' });
      } catch (e) { Object.assign(f, { path, p: null, error: String(e) }); }
    }
    draw();
  };
  const save = async () => {
    const p = f.p;
    if (!p) { f.error = 'Choose a folder first.'; return draw(); }
    if (p.taken) return draw();
    const port = Number(f.port);
    if (p.mode === 'proxy' && !(Number.isInteger(port) && port > 0 && port < 65536)) { f.error = f.port + ' is not a port (1 to 65535)'; return draw(); }
    Object.assign(f, { busy: true, error: '' });
    draw();
    const site = { site: p.site, cwd: '/' };
    try {
      await daemon({ cmd: 'link', name: null, path: f.path });
      if (p.mode === 'proxy' && port !== p.upstream) await daemon({ cmd: 'app_set', ...site, upstream: port });
      if (p.mode === 'php' && f.php !== st.default_php) await daemon({ cmd: 'isolate', version: f.php, ...site });
      closeDialog();
      toast(p.host + ' is linked', false, 'https://' + p.host + (p.mode === 'proxy' ? ' → localhost:' + port : p.mode === 'php' ? ' · PHP ' + f.php : ''));
      refresh(true);
    } catch (e) { Object.assign(f, { busy: false, error: String(e) }); draw(); }
  };
  const draw = () => showDialog(h('div.dv-dialog', { role: 'dialog', 'aria-modal': 'true', 'aria-labelledby': 'lk-t' },
    h('h2', { id: 'lk-t' }, 'Link a site'),
    h('p.dv-sub', {}, 'Any folder can be served, also outside your parked folders. Its name becomes the domain.'),
    h('div.lk-folder', {}, h('span.dv-avatar.folder', {}, icon('folder')),
      h('span.dv-grow.dv-mono.dv-ellipsis', { title: f.path }, f.path || 'No folder chosen yet'),
      h('button.dv-btn', { type: 'button', onclick: choose, disabled: f.busy, 'data-focus': !f.p }, 'Choose…')),
    f.p && h('div.lk-grid', {},
      h('span.dv-sub', {}, 'Domain'), h('span.dv-mono', {}, f.p.host),
      h('span.dv-sub', {}, 'Type'), h('span', {}, h('span.dv-chip.muted', {}, f.p.preset_label), f.p.detected ? '' : h('span.dv-sub', {}, ' nothing detected; served as PHP')),
      f.p.mode === 'proxy' && [h('label.dv-sub', { for: 'lk-port' }, 'Dev server port'),
        h('span.dv-line', {}, h('input.dv-input.dv-mono.port', { id: 'lk-port', value: f.port, inputmode: 'numeric', autocomplete: 'off',
          oninput: (e) => { f.port = e.target.value.trim(); }, onkeydown: (e) => { if (e.key === 'Enter') save(); } }),
        h('span.dv-sub', {}, 'https://' + f.p.host + ' forwards to it'))],
      f.p.mode === 'php' && [h('span.dv-sub', {}, 'PHP version'),
        h('span', {}, h('div.dv-seg', { role: 'group', 'aria-label': 'PHP version' }, st.php.map((x) => x.version).map((v) =>
          h('button', { type: 'button', 'aria-pressed': String(f.php === v), onclick: () => { f.php = v; draw(); } },
            v + (v === st.default_php ? ' (default)' : '')))))]),
    f.error ? h('p.dv-error', { role: 'alert' }, f.error) : null,
    h('div.dv-dialog-btns', {}, h('button.dv-btn', { type: 'button', onclick: closeDialog }, 'Cancel'),
      h('button.dv-btn.primary', { type: 'button', onclick: save, disabled: f.busy || !f.p || !!f.p.taken, 'data-focus': !!f.p }, f.busy ? 'Linking…' : 'Link site'))),
  closeDialog);
  draw();
}

async function parkFolder() {
  const path = await invoke('choose_folder', { purpose: 'park' }).catch((e) => toast(String(e), true));
  if (path) act('Parking…', () => daemon({ cmd: 'park', path }), 'Parked ' + path);
}

function sitesPage(d) {
  const st = S.status;
  const installed = st.php.map((p) => p.version);
  const ignored = d.ignored || [];
  const inside = (list, folder) => list.filter((x) => !x.linked && x.path.startsWith(folder + '/')).length;
  const out = [];
  if (st.server_status === 'stopped')
    out.push(h('div.dv-banner.warn', { role: 'status' }, h('span.dv-dot.warn'),
      text(h('span.dv-strong', {}, 'Devid is stopped'), 'Your sites are not being served right now.'), btn('Start all', startAll, 'primary')));

  out.push(h('div.dv-head', {},
    h('div', {}, h('h2.dv-h2', {}, 'Parked folders ', h('span.dv-light', {}, '(' + d.parked.length + ')')),
      h('div.dv-sub', {}, 'Every folder inside a parked folder is served automatically at ', h('b', {}, 'folder-name'), '.' + st.tld)),
    btn([icon('plus'), 'Add folder'], parkFolder)));
  out.push(d.parked.length ? card(d.parked.map((folder) => {
    const n = inside(d.sites, folder);
    const removed = inside(ignored, folder);
    return row(h('span.dv-avatar.folder', {}, icon('folder')),
      text(h('span.dv-mono', {}, folder), (n ? plural(n, 'site') + ' served from here' : 'No sites yet') + (removed ? ' · ' + removed + ' removed' : '')),
      moreBtn('Actions for ' + folder, () => [
        { header: folder },
        { label: 'Show in Finder', run: () => openTarget({ what: 'finder', path: folder }) },
        { label: 'Open folder in Terminal', run: () => openTarget({ what: 'terminal', path: folder }) },
        '-',
        { label: 'Remove parked folder…', danger: true, run: async () => {
          const msg = (n ? plural(n, 'site') + ' inside it stop being served.' : 'No sites inside it are served right now.') + ' The folder and your files are not touched.';
          if (await ask('Remove ' + folder + '?', msg, 'Remove', true))
            act('Removing…', () => daemon({ cmd: 'unpark', path: folder }), 'Unparked ' + folder);
        } },
      ]));
  })) : card(h('div.dv-empty', {}, h('div.dv-strong', {}, 'No parked folders'),
    h('div.dv-sub', {}, 'Add a folder like ~/Projects and every project inside is served automatically.'))));

  // Removed parked sites (D-029) sit behind the filter on the Sites line, in the same table.
  const removed = S.siteFilter === 'removed';
  out.push(h('div.dv-head', {}, h('h2.dv-h2', {}, 'Sites'),
    dropdown('Show sites', removed ? 'removed' : 'served',
      [{ value: 'served', label: 'Served', kbd: String(d.sites.length) }, { value: 'removed', label: 'Removed', kbd: String(ignored.length) }],
      (v) => { S.siteFilter = v; renderPage(); }, (removed ? 'Removed (' + ignored.length + ')' : 'Served (' + d.sites.length + ')'))));
  const rows = removed ? ignored.map(removedRow) : d.sites.map((s) => siteRow(s, installed, d.parked));
  out.push(h('div.st-grid.st-cols', { 'aria-hidden': 'true' }, h('span'), h('span', {}, 'Site'), h('span', {}, 'Serves'), h('span', {}, 'Processes'), h('span')));
  out.push(h('div.dv-card', { role: 'list' }, rows.length ? rows : h('div.dv-empty', {},
    h('div.dv-strong', {}, removed ? 'No removed sites' : 'No sites yet'),
    h('div.dv-sub', {}, removed ? 'Sites you remove from a parked folder show up here, so you can serve them again.' : 'Park a folder, or link one with the + button.'))));
  out.push(h('p.dv-sub.dv-note', {}, removed ? 'Removed sites are not served. Their folders are not touched.'
    : 'PHP sites pick their version; a change reloads only that site. Node sites are proxied to the port their dev server listens on.'));
  return out;
}

/** One line of the Sites table: the site, how it is served, its processes, then its menu and details. */
function siteRow(s, installed, parked) {
  const st = S.status;
  const host = siteHost(s);
  const running = (s.commands || []).filter((c) => c.state === 'running').length;
  const open = () => openTarget({ what: 'site', name: s.name });
  const versions = installed.includes(s.php) ? installed : [...installed, s.php];
  // The default version means following the default again; another one isolates the site.
  const usePhp = (version) => version === st.default_php
    ? act('Switching ' + s.name + '…', () => daemon({ cmd: 'unisolate', site: s.name, cwd: '/' }), host + ' uses the default PHP again')
    : act('Switching ' + s.name + '…', () => daemon({ cmd: 'isolate', version, site: s.name, cwd: '/' }), host + ' now uses PHP ' + version);
  const php = h('button.dv-select', { type: 'button', 'aria-label': 'PHP version for ' + host + ': ' + s.php, 'aria-haspopup': 'menu', 'aria-expanded': 'false', disabled: !!S.busy,
    onclick: (e) => menu(e.currentTarget, versions.map((v) => ({ label: 'PHP ' + v, kbd: v === st.default_php ? 'default' : '', checked: v === s.php,
      run: () => { if (v !== s.php || (v === st.default_php && s.isolated)) usePhp(v); } })), true) },
  h('span', {}, 'PHP ' + s.php), icon('chevron'));
  return h('div.st-grid.st-row', { role: 'listitem' },
    h('span.dv-avatar', { 'aria-hidden': 'true' }, s.name.charAt(0).toUpperCase()),
    h('div.st-site', {},
      h('div.dv-line', {},
        h('button.dv-host', { type: 'button', title: 'Open ' + s.url, onclick: open }, host),
        h('span.dv-chip.muted', {}, s.preset_label),
        s.mode === 'php' && s.isolated ? h('span.dv-chip.accent', { title: 'This site uses its own PHP version' }, 'Isolated') : null,
        s.mode !== 'php' || s.php_installed ? null : h('span.dv-chip.warn', {}, 'PHP ' + s.php + ' not installed'),
        s.share?.url ? h('span.dv-chip.accent', { title: 'Shared at ' + s.share.url }, 'Shared') : null),
      h('div.dv-sub.st-path', { title: s.path }, h('span.dv-mono.dv-ellipsis', {}, s.path), s.linked ? h('span.dv-nowrap', {}, ' · Linked') : null)),
    h('div.st-serve', {}, s.mode === 'php' ? php : h('span.st-label', {}, s.mode === 'proxy' ? ['Proxy ', h('span.dv-mono', {}, ':' + s.upstream)] : 'Static')),
    h('div.st-proc', {},
      running ? h('span.st-run', {}, h('i', { 'aria-hidden': 'true' }), running + ' running') : h('span.dv-sub', {}, 'Not running'),
      s.port ? h('span.dv-sub.dv-mono' + (s.port_busy ? '.st-busy' : ''), { title: s.port_busy ? 'Port ' + s.port + ' is in use by ' + s.port_busy
        : 'Also served on http://127.0.0.1:' + s.port }, 'on :' + s.port + (s.port_busy ? ' · busy' : '')) : null),
    h('div.st-acts', {},
      moreBtn('More actions for ' + host, () => [
        { header: host },
        { label: 'Open in browser', run: open },
        { label: 'Copy URL', run: () => act('Copying…', () => invoke('copy_url', { name: s.name }), (m) => m) },
        { label: 'Open folder in Terminal', run: () => openTarget({ what: 'terminal', path: s.path }) },
        { label: 'Details', run: () => openSite(s.name) },
        '-',
        s.linked ? { label: 'Unlink…', danger: true, run: async () => {
          if (await ask('Unlink ' + host + '?', 'The folder itself is not touched.', 'Unlink', true))
            act('Unlinking…', () => daemon({ cmd: 'unlink', name: s.name }), 'Unlinked ' + s.name);
        } } : { label: 'Remove site…', danger: true, run: async () => {
          const folder = parked.find((f) => s.path.startsWith(f + '/')) || '';
          if (await ask('Remove ' + host + '?', 'Devid stops serving it and ignores ' + s.path + ' inside ' + folder + '; the folder itself is not touched.', 'Remove', true))
            act('Removing…', () => daemon({ cmd: 'ignore', site: s.name }), (r) => 'Removed ' + host + '; find it under Removed to serve it again');
        } },
      ]),
      h('button.dv-btn.ghost.icon', { type: 'button', title: 'Details', 'aria-label': 'Details for ' + host, onclick: () => openSite(s.name) }, icon('next'))));
}

function removedRow(i) {
  const host = i.name + '.' + S.status.tld;
  return h('div.st-grid.st-row', { role: 'listitem' },
    h('span.dv-avatar', { 'aria-hidden': 'true' }, i.name.charAt(0).toUpperCase()),
    h('div.st-site', {}, h('div', {}, host), h('div.dv-sub.dv-ellipsis', { title: i.path }, h('span.dv-mono', {}, i.path))),
    h('div.st-serve', {}, h('span.dv-sub', {}, 'Not served')), h('span'),
    h('div.st-acts', {}, moreBtn('More actions for ' + host, () => [
      { header: host },
      { label: 'Open folder in Terminal', run: () => openTarget({ what: 'terminal', path: i.path }) },
      { label: 'Serve again', run: () => act('Restoring…', () => daemon({ cmd: 'unignore', site: i.name }), 'Serving ' + host + ' again') },
    ])));
}

/** The status card's ⋯: the sites, Start/Stop, the default PHP, Settings and Quit, beside the card. */
async function quickMenu(anchor) {
  const st = S.status;
  let sites = [];
  try { sites = (await daemon({ cmd: 'sites' })).sites; } catch (e) { toast(String(e), true); }
  const running = st.server_status === 'running';
  menu(anchor, [
    { header: running ? st.server + ' · PHP ' + st.default_php + ' · Running' : 'Devid is stopped', dot: running ? 'ok' : 'warn' },
    sites.length && '-',
    sites.length && { header: 'Sites' },
    ...sites.map((s) => ({ label: siteHost(s), kbd: s.mode === 'php' ? 'PHP ' + s.php : s.mode === 'proxy' ? ':' + s.upstream : 'Static',
      run: () => openTarget({ what: 'site', name: s.name }) })),
    '-',
    ...(running ? [{ label: 'Stop all', run: stopAll }, { label: 'Restart ' + st.server, run: () => restart(st.server) }] : [{ label: 'Start all', run: startAll }]),
    '-',
    { header: 'Default PHP' },
    { row: st.php.map((p) => ({ label: p.version, checked: p.version === st.default_php,
      run: () => { if (p.version !== st.default_php) act('Switching…', () => daemon({ cmd: 'php_use', version: p.version }), 'PHP ' + p.version + ' is now the default'); } })),
    label: 'Default PHP' },
    '-',
    { label: 'Settings…', run: () => go('settings') },
    { label: 'Quit Devid', run: () => invoke('quit') },
  ], false, anchor.closest('.dv-statuscard'));
}

// ---------- site detail (Phase 8 A5: app type D-035, local port D-037, commands D-036) ----------

const siteHost = (s) => new URL(s.url).hostname;
const presetLabel = (key) => (PRESETS.find((p) => p[0] === key) || [key, key])[1];
/** The open site detail's site from the last `sites` reply; a site that is gone closes it. */
function openedSite() {
  const s = S.site && S.data.sites?.sites.find((x) => x.name === S.site);
  if (S.site && S.data.sites && !s) S.site = null;
  return s || null;
}

function openSite(name) {
  S.site = name;
  S.logPanel = null;
  closeMenu();
  $('page').parentElement.scrollTop = 0;
  renderChrome();
  renderPage();
}

/** A text field that saves on Enter or when it loses focus. */
function field(id, value, save, cls = '', extra = {}) {
  return h('input.dv-input' + (cls ? '.' + cls : ''), { id, value: value ?? '', spellcheck: 'false', autocomplete: 'off', disabled: !!S.busy,
    onchange: (e) => save(e.target.value.trim()), onkeydown: (e) => { if (e.key === 'Enter') e.target.blur(); }, ...extra });
}

const label = (id, title, sub) => h('label.dv-grow', { for: id }, h('div', {}, title), h('div.dv-sub', {}, sub));

function asPort(v, run) {
  const n = Number(v);
  if (Number.isInteger(n) && n > 0 && n < 65536) run(n);
  else { toast(v + ' is not a port (1 to 65535)', true); renderPage(); }
}

function siteDetail(s) {
  const st = S.status;
  const host = siteHost(s);
  const site = { site: s.name, cwd: '/' };
  const set = (fields, done) => act('Saving…', () => daemon({ cmd: 'app_set', ...site, ...fields }), done);
  const rel = s.docroot === s.path ? '.' : s.docroot.startsWith(s.path + '/') ? s.docroot.slice(s.path.length + 1) : s.docroot;
  const typeSet = !!s.app?.preset;
  const installed = st.php.map((p) => p.version);
  const versions = installed.includes(s.php) ? installed : [...installed, s.php];
  const port = (p, busy, done) => act(busy, () => daemon({ cmd: 'app_port', ...site, port: p }), done);

  const app = card(
    row(text('Type', typeSet ? 'Set by you' + (s.detected ? '; detected: ' + presetLabel(s.detected) : '; nothing detected')
      : s.detected ? 'Detected from the project files' : 'Nothing detected; served as PHP, as before'),
    typeSet ? btn('Use detected', () => act('Saving…', () => daemon({ cmd: 'app_reset', ...site }), host + ' follows detection again'), 'ghost') : null,
    dropdown('Type', s.preset, PRESETS.map(([key, name]) => ({ value: key, label: name + (key === s.detected ? ' (detected)' : '') })),
      (v) => set({ preset: v }, host + ' is now ' + presetLabel(v)))),
    row(text('Serve as', { php: '.php files go to php-fpm; other files are served as they are.',
      static: 'Files only, from the web root; no PHP runs.',
      proxy: 'Every request goes to the dev server on localhost:' + s.upstream + ', WebSockets included.' }[s.mode]),
    h('div.dv-seg', { role: 'group', 'aria-label': 'Serve as' }, Object.entries(MODES).map(([m, name]) =>
      h('button', { type: 'button', 'aria-pressed': String(s.mode === m), disabled: !!S.busy,
        onclick: () => { if (s.mode !== m) set({ mode: m }, host + ' is served as ' + name); } }, name)))),
    s.mode === 'php' && row(text('PHP version', !s.php_installed ? 'PHP ' + s.php + ' is not installed; install it on the PHP page.'
      : s.isolated ? 'This site\'s own version' : 'The default version'),
      dropdown('PHP version for ' + host, s.php, versions.map((v) => ({ value: v, label: 'PHP ' + v + (v === st.default_php ? ' (default)' : '') })),
        (version) => act('Switching…', () => daemon({ cmd: 'isolate', version, ...site }), host + ' now uses PHP ' + version))),
    s.mode !== 'proxy' && row(label('app-root', 'Web root', 'Inside the project folder; "." is the folder itself.'),
      field('app-root', rel, (v) => { if (v && v !== rel) set({ root: v }, 'Web root: ' + v); }, 'dv-mono')),
    s.mode === 'static' && row(label('app-spa', 'Single-page app', 'Paths with no file load index.html, for client-side routing.'),
      h('input.dv-switch', { id: 'app-spa', type: 'checkbox', role: 'switch', checked: !!s.spa, disabled: !!S.busy,
        onchange: (e) => set({ spa: e.target.checked }, e.target.checked ? 'Single-page app on' : 'Single-page app off') })),
    s.mode === 'proxy' && row(label('app-up', 'Forward to', 'The port the dev server listens on. Commands get it as PORT.'), h('span.dv-sub.dv-mono', {}, 'localhost:'),
      field('app-up', String(s.upstream), (v) => asPort(v, (n) => { if (n !== s.upstream) set({ upstream: n }, 'Forwarding to localhost:' + n); }),
        'dv-mono.port', { inputmode: 'numeric' })));

  const local = card(
    row(label('app-porton', 'Also serve on a local port', 'Plain HTTP on 127.0.0.1, for a tunnel or a tool that needs a fixed port.'),
      h('input.dv-switch', { id: 'app-porton', type: 'checkbox', role: 'switch', checked: !!s.port, disabled: !!S.busy,
        onchange: (e) => port(e.target.checked ? null : 0, 'Saving…', (r) => r.port ? host + ' is also served on 127.0.0.1:' + r.port : 'Local port off') })),
    s.port && row(label('app-port', 'Port', 'http://127.0.0.1:' + s.port),
      s.port_busy ? h('span.dv-chip.warn', {}, 'In use') : h('span.dv-chip.ok', {}, 'Serving'),
      field('app-port', String(s.port), (v) => asPort(v, (n) => { if (n !== s.port) port(n, 'Saving…', host + ' is also served on 127.0.0.1:' + n); }),
        'dv-mono.port', { inputmode: 'numeric' })),
    s.port_busy && row(h('div.dv-grow.dv-sub', { role: 'status' }, s.port_busy + ' is listening on ' + s.port +
      '. Devid leaves the port out until it is free; your other sites are not affected.'),
    btn('Check again', () => port(null, 'Checking…', 'Checked port ' + s.port))));

  const cmds = s.commands || [];
  const running = cmds.filter((c) => c.state === 'running').length;
  const req = (cmd, name) => daemon({ cmd, ...site, name: name ?? null });
  const started = (r) => {
    if (r.warning) toast(r.warning, true);
    const bad = r.results.filter((x) => !x.ok);
    if (bad.length) toast(bad.map((x) => x.name + ': ' + x.error).join('; '), true);
    const ok = r.results.filter((x) => x.ok).map((x) => x.name);
    return ok.length ? 'Started ' + andList(ok) : '';
  };
  const panel = S.logPanel && cmds.find((c) => c.name === S.logPanel);
  let box = null;
  if (panel) {
    if (S.log.name !== panel.log) pickLog(panel.log, false);
    box = h('div.dv-log.is-panel', { id: 'logbox', role: 'log', tabindex: '0', 'aria-label': 'Log of ' + panel.name });
    fillLog(box);
  }
  return [
    h('h2.dv-h2', {}, 'App'), app,
    h('p.dv-sub.dv-note', {}, 'https://' + host + ' keeps its certificate from the Devid CA in every mode.'),
    h('h2.dv-h2', {}, 'Local port'), local,
    h('div.dv-head', {}, h('h2.dv-h2', {}, 'Commands ', h('span.dv-light', {}, '(' + cmds.length + ')')),
      h('div.dv-tools', {},
        cmds.length ? btn([icon('play'), 'Start all'], () => act('Starting…', () => req('command_start'), started), '', { disabled: !!S.busy || running === cmds.length }) : null,
        cmds.length ? btn([icon('stop'), 'Stop all'], () => act('Stopping…', () => req('command_stop'),
          (r) => r.stopped.length ? 'Stopped ' + andList(r.stopped) : 'Nothing was running'), '', { disabled: !!S.busy || !running }) : null,
        btn([icon('plus'), 'Add command'], () => addCommand(s), 'primary'))),
    card(cmds.length ? cmds.map((c) => commandRow(c, req)) : h('div.dv-empty', {}, h('div.dv-strong', {}, 'No commands yet'),
      h('div.dv-sub', {}, 'Add the commands you run in Terminal for this project, such as a dev server or a queue worker.'))),
    panel && h('div.dv-logbar', {}, h('span.dv-strong', {}, 'Log'),
      h('span.dv-sub.dv-mono.dv-grow.dv-ellipsis', {}, (st.home || '~/.devid') + '/log/' + panel.log),
      btn('Hide', () => { S.logPanel = null; renderPage(); }, 'ghost')),
    box,
    h('p.dv-sub.dv-note', {}, 'Commands run in ', h('span.dv-mono', {}, s.path), ' with your login shell\'s PATH (nvm, ~/.cargo/bin). ' +
      'They start only when you press Start, never at login. A command that asks a question stops with an error; its log says why.'),
    h('h2.dv-h2', {}, 'Share'), shareCard(s, host),
    h('p.dv-sub.dv-note', {}, 'Uses the cloudflared already on this Mac. Your own Cloudflare tunnel is never touched. ' +
      'Links the app builds from APP_URL still point at ' + host + '.'),
  ];
}

/** Share (D-037): a temporary trycloudflare.com link to the site, until Stop sharing. */
function shareCard(s, host) {
  const sh = s.share;
  const on = sh && (sh.state === 'running' || sh.state === 'starting');
  const req = (cmd) => daemon({ cmd, site: s.name, cwd: '/' });
  return card(
    row(text('Share on the internet', 'A temporary trycloudflare.com link to ' + host + '. Anyone with the link can open the site until you stop sharing.'),
      sh?.state === 'starting' ? h('span.dv-sub', { role: 'status' }, 'Connecting…') : null,
      on ? btn('Stop sharing', () => act('Stopping…', () => req('share_stop'), 'Stopped sharing ' + host))
        : busyBtn('share', 'Share', () => act('Starting cloudflared…', () => req('share_start'),
          (r) => r.url ? 'Shared at ' + r.url : 'cloudflared is still connecting; the link shows here', 'share'))),
    sh?.url && row(h('span.dv-dot.ok', { 'aria-hidden': 'true' }), h('div.dv-grow.dv-mono.dv-ellipsis.dv-select-text', {}, sh.url),
      btn('Copy', () => act('Copying…', () => invoke('copy_url', { name: s.name, share: true }), (m) => m))),
    sh?.state === 'exited' && row(h('div.dv-grow.dv-sub', { role: 'alert' }, 'cloudflared stopped. Its log, ' + sh.log + ', is on the Logs page.'),
      busyBtn('share', 'Share again', () => act('Starting cloudflared…', () => req('share_start'),
        (r) => r.url ? 'Shared at ' + r.url : 'cloudflared is still connecting; the link shows here', 'share'))));
}

function commandRow(c, req) {
  const on = c.state === 'running';
  const failed = c.state === 'exited' && c.code !== 0;
  const state = on ? 'Running · pid ' + c.pid
    : c.state === 'exited' ? (c.code != null ? 'Exited with code ' + c.code : 'Ended by signal ' + c.signal) : 'Stopped';
  const logOpen = S.logPanel === c.name;
  const toggleLog = () => { S.logPanel = logOpen ? null : c.name; renderPage(); };
  const start = () => act('Starting ' + c.name + '…', () => req('command_start', c.name), (r) => {
    if (r.warning) toast(r.warning, true);
    return 'Started ' + c.name;
  });
  const stop = () => act('Stopping ' + c.name + '…', () => req('command_stop', c.name), 'Stopped ' + c.name);
  return row(h('span.dv-dot' + (on ? '.ok' : failed ? '.bad' : ''), { 'aria-hidden': 'true' }),
    h('div.dv-grow', {},
      h('div.dv-line', {}, h('span.dv-mono.dv-strong', {}, c.name), h('span.dv-sub', { role: 'status' }, state)),
      h('div.dv-sub.dv-mono.dv-ellipsis', {}, c.run)),
    on ? btn([icon('stop'), 'Stop'], stop, '', { 'aria-label': 'Stop ' + c.name })
      : btn([icon('play'), 'Start'], start, '', { 'aria-label': 'Start ' + c.name }),
    h('button.dv-btn.ghost.icon', { type: 'button', 'aria-pressed': String(logOpen), 'aria-label': 'Log of ' + c.name, title: 'Log', onclick: toggleLog }, icon('logs')),
    moreBtn('More for ' + c.name, () => [
      { header: c.name },
      on && { label: 'Restart', run: () => act('Restarting ' + c.name + '…', async () => {
        await req('command_stop', c.name);
        return req('command_start', c.name);
      }, 'Restarted ' + c.name) },
      { label: logOpen ? 'Hide log' : 'Show log', run: toggleLog },
      '-',
      { label: 'Remove…', danger: true, run: async () => {
        if (await ask('Remove ' + c.name + '?', (on ? 'It is stopped first. ' : '') + 'Devid forgets the command; its log stays as ' + c.log + '.', 'Remove', true))
          act('Removing…', () => req('command_remove', c.name), 'Removed ' + c.name);
      } },
    ]));
}

/** The prototype's "Add a command" dialog: a name, a shell line, and the site's suggestions. */
function addCommand(s) {
  const f = { name: '', run: '', error: '' };
  const save = async () => {
    const name = f.name.trim();
    const run = f.run.trim();
    if (!name || !run) { f.error = 'Fill in a name and a command.'; draw(); return; }
    try {
      await daemon({ cmd: 'command_add', site: s.name, cwd: '/', name, run });
      closeDialog();
      toast('Added ' + name + '; press Start to run it');
      refresh(true);
    } catch (e) { f.error = String(e); draw(); }
  };
  const draw = () => showDialog(h('div.dv-dialog', { role: 'dialog', 'aria-modal': 'true', 'aria-labelledby': 'add-t' },
    h('h2', { id: 'add-t' }, 'Add a command to ' + siteHost(s)),
    h('p.dv-sub', {}, 'It runs in ' + s.path + ' and stays stopped until you press Start.'),
    h('div.dv-form', {},
      h('label.dv-field', {}, 'Name', h('input.dv-input.dv-mono', { value: f.name, placeholder: 'dev', 'aria-label': 'Name', 'data-focus': true,
        spellcheck: 'false', autocomplete: 'off', oninput: (e) => { f.name = e.target.value; } })),
      h('label.dv-field', {}, 'Command', h('input.dv-input.dv-mono', { value: f.run, placeholder: 'npm run dev', 'aria-label': 'Command',
        spellcheck: 'false', autocomplete: 'off', oninput: (e) => { f.run = e.target.value; }, onkeydown: (e) => { if (e.key === 'Enter') save(); } }))),
    s.suggestions?.length ? h('div.dv-sugg', {}, h('div.dv-sub', {}, 'Suggestions from this project'),
      h('div.dv-sugg-list', {}, s.suggestions.map((x) => h('button.dv-btn' + (f.run === x.run ? '.primary' : ''), { type: 'button',
        'aria-pressed': String(f.run === x.run), 'aria-label': 'Use ' + x.run, title: x.run,
        onclick: () => { Object.assign(f, { name: x.name, run: x.run, error: '' }); draw(); } }, x.name)))) : null,
    f.error ? h('p.dv-error', { role: 'alert' }, f.error) : null,
    h('div.dv-dialog-btns', {}, h('button.dv-btn', { type: 'button', onclick: closeDialog }, 'Cancel'),
      h('button.dv-btn.primary', { type: 'button', onclick: save }, 'Add command'))), closeDialog);
  draw();
}

function phpPage(d) {
  const st = S.status;
  const installed = d.php.map((p) => p.version);
  const def = dropdown('Default PHP version', st.default_php, installed.map((v) => ({ value: v, label: 'PHP ' + v })), (version) =>
    act('Switching…', () => daemon({ cmd: 'php_use', version }), 'PHP ' + version + ' is now the default'));
  return [
    h('h2.dv-h2', {}, 'Default version'),
    card(row(text('Default PHP version', ['Used by sites that are not isolated, and by ', h('span.dv-mono', {}, 'php'), ' in Terminal']), def)),
    h('h2.dv-h2', {}, 'Versions'),
    card(SUPPORTED_PHP.map((v) => {
      const p = d.php.find((x) => x.version === v);
      if (!p)
        return row(h('span.dv-vbadge.off', {}, v), text(h('span.dv-strong', {}, 'PHP ' + v), 'Not installed'),
          busyBtn('php-' + v, 'Install', () => act('Installing…', () => daemon({ cmd: 'php_install', version: v, from: null }), 'PHP ' + v + ' installed', 'php-' + v), 'primary'));
      const service = 'php-' + v;
      return row(h('span.dv-vbadge', {}, v),
        h('div.dv-grow', {}, h('div.dv-line', {}, h('span.dv-strong', {}, 'PHP ' + v),
          p.default ? h('span.dv-chip.accent', {}, 'Default') : null,
          p.status === 'failed' ? h('span.dv-chip.bad', {}, 'php-fpm failed') : null),
        h('div.dv-sub', {}, 'php-fpm ' + p.status + (p.pid ? ' (pid ' + p.pid + ')' : ''))),
        btn('Extensions', () => openExtensions(v)),
        btn('php.ini', () => act('Opening…', () => invoke('open', { target: { what: 'php_ini', version: v } }),
          (m) => { toast(m, false, 'Restart php-fpm ' + v + ' after you save it.'); })),
        btn('Restart', () => restart(service)));
    })),
    h('p.dv-sub.dv-note', {}, 'Static builds for Intel and Apple silicon Macs. Updates are in Settings.'),
  ];
}

function serverPage(d) {
  const st = S.status;
  const active = d.servers.find((s) => s.active) || {};
  const label = (k) => SERVERS[k]?.label || k;
  const running = active.status === 'running';
  const ports = portsText(st);
  return [
    h('div.dv-card', {}, row(
      h('span.dv-dot.' + (running ? 'ok' : active.status === 'failed' ? 'bad' : 'warn')),
      text(h('span.dv-strong', {}, label(active.server) + ' ' + (active.version || '')), capital(active.status || 'unknown')),
      btn('Restart', () => restart(active.server)),
      active.paused
        ? btn('Start', () => act('Starting ' + active.server + '…', () => daemon({ cmd: 'service_start', service: active.server }), 'Started ' + active.server, 'card'))
        : btn('Stop', () => act('Stopping ' + active.server + '…', () => daemon({ cmd: 'service_stop', service: active.server }),
          'Stopped ' + active.server + ' (sites are down until you start it)', 'card')))),
    h('h2.dv-h2', {}, 'Web server'),
    h('div.dv-choice', {}, d.servers.map((s) => h('div.dv-choice-card' + (s.active ? '.is-selected' : ''), {},
      h('div.dv-line', {}, h('b', {}, label(s.server)), h('span.dv-sub.dv-mono', {}, s.version || ''),
        SERVERS[s.server]?.recommended ? h('span.dv-chip.accent', {}, 'Recommended') : null),
      h('div.dv-sub', {}, SERVERS[s.server]?.hint || ''),
      h('div.dv-foot', {},
        s.active ? h('span.dv-chip.ok', {}, '● Active') :
        s.installed ? btn('Use', async () => {
          if (await ask('Switch to ' + label(s.server) + '?', 'All ' + plural(st.sites, 'site') + ' restart on ' + label(s.server) + '.', 'Switch to ' + label(s.server)))
            act('Switching to ' + label(s.server) + '…', () => daemon({ cmd: 'server_use', server: s.server }), 'Now serving with ' + s.server, 'card');
        }) :
        busyBtn('server-' + s.server, s.server === 'apache' ? 'Use macOS httpd' : 'Install', () =>
          act('Installing…', () => daemon({ cmd: 'server_install', server: s.server, from: null }), s.server + ' installed', 'server-' + s.server)))))),
    h('h2.dv-h2', {}, 'Health'),
    card(
      row(text('Ports'), h('span.dv-sub.dv-mono', {}, ports)),
      row(text('DNS *.' + st.tld, 'Answers every .' + st.tld + ' domain with 127.0.0.1'),
        h('span.dv-chip.' + (st.dns.startsWith('listening') ? 'ok' : 'bad'), {}, capital(st.dns)))),
    h('h2.dv-h2', {}, 'Configuration'),
    card(row(text('Main config', [h('span.dv-mono', {}, active.config || ''), h('br'), 'Devid rewrites this file on every change.']),
      btn('View config', () => viewConfig(active.server)))),
  ];
}

async function viewConfig(server) {
  let c;
  try { c = await invoke('server_config', { server }); } catch (e) { return toast(String(e), true); }
  const label = SERVERS[server]?.label || server;
  showDialog(h('div.dv-dialog.wide', { role: 'dialog', 'aria-modal': 'true', 'aria-labelledby': 'cf-t' },
    h('h2', { id: 'cf-t' }, label + ' config'),
    h('p.dv-sub.dv-mono', {}, c.path),
    h('p.dv-sub', {}, 'Read-only. Devid rewrites this file on every change.'),
    h('pre.dv-pre', {}, c.text),
    h('div.dv-dialog-btns', {},
      h('button.dv-btn', { type: 'button', onclick: () => openTarget({ what: 'finder', path: c.path }) }, 'Show in Finder'),
      h('button.dv-btn.primary', { type: 'button', 'data-focus': true, onclick: closeDialog }, 'Close'))), closeDialog);
}

// ---------- Logs: sources on the left, the open log on the right ----------

/** Log names, the sites' commands and what is running. Sizes only feed the badges: in the page's
 * key they would redraw it on every poll. */
async function loadLogs() {
  const [list, sites] = await Promise.all([daemon({ cmd: 'logs' }), daemon({ cmd: 'sites' })]);
  await countErrors(list.logs);
  const st = S.status;
  return {
    logs: list.logs.map((l) => l.name),
    sites: sites.sites.map((s) => ({ name: s.name, share: s.share?.state || null,
      commands: (s.commands || []).map((c) => ({ name: c.name, log: c.log, on: c.state === 'running' })) })),
    running: [st.server, st.server_status, st.php.map((p) => p.version + ' ' + p.status)],
    errors: { ...S.log.errors },
  };
}

/** Reads what the other logs gained since the last poll and counts its error lines. */
async function countErrors(list) {
  const L = S.log;
  for (const { name, size } of list) {
    const seen = L.seen[name];
    if (seen === undefined || name === L.name) { L.seen[name] = size; delete L.errors[name]; continue; }
    if (size === seen) continue;
    let r;
    try { r = await daemon({ cmd: 'log_read', name, offset: size < seen ? 0 : seen }); } catch { continue; }
    const n = r.text.split('\n').filter((t) => lineLevel(t) === 'error').length;
    if (n) L.errors[name] = (L.errors[name] || 0) + n;
    L.seen[name] = r.offset;
  }
}

const lineLevel = (t) => /error|crit|fatal|emerg|alert/i.test(t) || /\bFAIL\b/.test(t) ? 'error' : /warn/i.test(t) ? 'warning' : '';
/** A line as a terminal leaves it: a progress bar (cargo, npm) redraws it after each \r. A final
 * \r stays, since the \n of a \r\n may come with the next read. */
const redraw = (t) => t.slice(t.lastIndexOf('\r', t.length - 2) + 1);

/** Every log as a source: Devid, the web server, PHP, each site's commands and share, then the rest. */
function logSources(d) {
  const st = S.status;
  const left = new Set(d.logs);
  const take = (name) => left.delete(name);
  const groups = [{ label: 'Devid', items: [] }, { label: 'Web server', items: [] }, { label: 'PHP', items: [] }];
  if (take('daemon.log')) groups[0].items.push({ name: 'daemon.log', label: 'Daemon', on: true });
  for (const name of d.logs) {
    const web = /^(nginx|caddy|apache)-(\w+)\.log$/.exec(name);
    if (web && take(name)) groups[1].items.push({ name, label: (SERVERS[web[1]]?.label || web[1]) + ' · ' + (web[2] === 'error' ? 'errors' : web[2]),
      on: st.server === web[1] && st.server_status === 'running' });
    const php = /^php(\d)(\d+)-fpm\.log$/.exec(name);
    if (php && take(name)) {
      const version = php[1] + '.' + php[2];
      groups[2].items.unshift({ name, label: 'PHP ' + version + ' · php-fpm', on: st.php.some((p) => p.version === version && p.status === 'running') });
    }
  }
  const sites = d.sites.map((s) => {
    const items = s.commands.filter((c) => take(c.log)).map((c) => ({ name: c.log, label: c.name, on: c.on, site: s.name }));
    return { name: s.name, items, share: 'share-' + s.name + '.log' };
  });
  // Logs of removed commands: the longest site name wins, since names may contain "-".
  for (const name of [...left].filter((n) => n.startsWith('app-'))) {
    const s = sites.filter((x) => name.startsWith('app-' + x.name + '-')).sort((a, b) => b.name.length - a.name.length)[0];
    if (s && take(name)) s.items.push({ name, label: name.slice(5 + s.name.length, -4), on: false, site: s.name });
  }
  for (const s of sites) {
    const share = d.sites.find((x) => x.name === s.name).share;
    if (take(s.share)) s.items.push({ name: s.share, label: 'share', on: share === 'running' || share === 'starting', site: s.name });
  }
  groups.push({ label: 'Other', items: [...left].map((name) => ({ name, label: name })) });
  return { groups: groups.filter((g) => g.items.length), sites: sites.filter((s) => s.items.length) };
}

function logsPage(d) {
  const L = S.log;
  if (!d.logs.includes(L.name)) {
    const php = 'php' + (S.status.default_php || '').replace('.', '') + '-fpm.log';
    pickLog(d.logs.includes(php) ? php : d.logs[0] || null, false);
  }
  if (!L.name) return [card(h('div.dv-empty.dv-sub', {}, 'No log files yet.'))];
  const src = logSources(d);
  const cur = [...src.groups, ...src.sites].flatMap((g) => g.items).find((x) => x.name === L.name) || { name: L.name, label: L.name };
  const title = cur.site ? cur.site + ' · ' + cur.label : cur.label;
  const path = (S.status.home || '~/.devid') + '/log/' + L.name;
  const box = h('div.dv-log.lg-log', { id: 'logbox', role: 'log', tabindex: '0', 'aria-label': title, onscroll: (e) => {
    const el = e.currentTarget;
    if (!el.isConnected) return; // a redraw replaced it; all its sizes read 0
    const atEnd = el.scrollHeight - el.scrollTop - el.clientHeight < 24;
    if (atEnd !== L.follow) { L.follow = atEnd; if (atEnd) L.newBelow = 0; syncFollow(); }
  } });
  fillLog(box);
  return h('div.lg-body', {},
    h('div.lg-src', {},
      h('label.dv-search.lg-src-search', {}, icon('search'), h('input.dv-input', { type: 'search', placeholder: 'Filter sources', 'aria-label': 'Filter log sources',
        value: L.srcQuery, spellcheck: 'false', autocomplete: 'off', oninput: (e) => { L.srcQuery = e.target.value; $('lg-list').replaceChildren(...sourceList(src)); } })),
      h('nav.lg-list', { id: 'lg-list', 'aria-label': 'Log sources' }, sourceList(src))),
    h('div.lg-view', {},
      h('div.lg-bar', {},
        h('div.dv-grow', {}, h('div.dv-strong.dv-ellipsis', {}, title),
          h('div.dv-sub.dv-mono.dv-ellipsis', { title: path }, path + ('on' in cur ? ' · ' + (cur.on ? 'running' : 'stopped') : ''))),
        moreBtn('More actions for this log', () => [
          { label: 'Show in Finder', run: () => openTarget({ what: 'finder', path }) },
          { label: 'Copy path', run: () => act('Copying…', () => invoke('copy_log_path', { name: L.name }), (m) => m) },
          '-',
          { label: 'Clear log…', danger: true, run: async () => {
            const name = L.name;
            if (await ask('Clear ' + name + '?', 'Its contents are deleted.', 'Clear', true))
              act('Clearing…', () => daemon({ cmd: 'log_clear', name }), () => { pickLog(name, false); return 'Cleared ' + name; });
          } },
        ])),
      h('div.lg-tools', {},
        h('div.dv-seg', { role: 'group', 'aria-label': 'Level' }, [['all', 'All'], ['warn', 'Warnings'], ['error', 'Errors']].map(([k, name]) =>
          h('button', { type: 'button', 'aria-pressed': String(L.level === k), onclick: () => { L.level = k; renderPage(); } }, name))),
        h('label.dv-search.dv-grow', {}, icon('search'), h('input.dv-input', { type: 'search', placeholder: 'Search in this log', 'aria-label': 'Search in this log',
          value: L.query, spellcheck: 'false', autocomplete: 'off', oninput: (e) => { L.query = e.target.value; fillLog($('logbox')); } }),
        h('span.dv-sub.dv-nowrap', { id: 'lg-count', 'aria-live': 'polite' })),
        h('label.dv-label', {}, 'Follow', h('input.dv-switch', { id: 'lg-follow', type: 'checkbox', role: 'switch', checked: L.follow,
          onchange: (e) => { L.follow = e.target.checked; L.newBelow = 0; syncFollow(); } }))),
      box,
      h('button.lg-jump', { type: 'button', id: 'lg-jump', hidden: L.follow, onclick: () => { L.follow = true; L.newBelow = 0; syncFollow(); } }, jumpText())));
}

function sourceList(src) {
  const L = S.log;
  const q = L.srcQuery.trim().toLowerCase();
  const match = (x) => !q || (x.label + ' ' + (x.site || '') + ' ' + x.name).toLowerCase().includes(q);
  const badge = (n) => n ? h('span.lg-badge', { 'aria-label': plural(n, 'new error') }, String(n)) : null;
  const item = (x, inSite) => h('button.lg-item' + (inSite ? '.in-site' : '') + (x.name === L.name ? '.is-active' : ''), { type: 'button', title: x.name,
    'aria-current': x.name === L.name ? 'true' : null, onclick: () => pickLog(x.name, true) },
  h('span.lg-sdot' + (x.on ? '.on' : ''), { 'aria-hidden': 'true' }), h('span.dv-grow.dv-ellipsis', {}, x.label), badge(L.errors[x.name]));
  const out = [];
  for (const g of src.groups) {
    const items = g.items.filter(match);
    if (items.length) out.push(h('div.lg-group', {}, g.label), items.map((x) => item(x, false)));
  }
  const sites = src.sites.map((s) => ({ ...s, shown: s.items.filter((x) => match({ ...x, site: s.name })) })).filter((s) => s.shown.length);
  if (sites.length) out.push(h('div.lg-group', {}, 'Sites'));
  for (const s of sites) {
    const open = q ? true : L.open[s.name] ?? s.items.some((x) => x.name === L.name);
    const running = s.items.filter((x) => x.on).length;
    const caret = icon('next');
    caret.classList.add('lg-caret');
    if (open) caret.classList.add('is-open');
    out.push(h('button.lg-site', { type: 'button', 'aria-expanded': String(open), onclick: () => { L.open[s.name] = !open; $('lg-list').replaceChildren(...sourceList(src)); } },
      caret, h('span.dv-grow.dv-ellipsis', {}, s.name), open ? null : h('span.dv-sub', {}, running ? running + ' running' : 'stopped'),
      open ? null : badge(s.items.reduce((n, x) => n + (L.errors[x.name] || 0), 0))));
    if (open) out.push(s.shown.map((x) => item(x, true)));
  }
  if (!out.length) out.push(h('div.lg-none.dv-sub', {}, 'No sources match “' + L.srcQuery.trim() + '”.'));
  return out.flat();
}

const jumpText = () => '↓ Jump to latest' + (S.log.newBelow ? ' · ' + S.log.newBelow + ' new' : '');

/** Follow on: the log stays at its end. Off (by the switch or by scrolling up): new lines are counted. */
function syncFollow() {
  const L = S.log;
  if ($('lg-follow')) $('lg-follow').checked = L.follow;
  if ($('lg-jump')) { $('lg-jump').hidden = L.follow; $('lg-jump').textContent = jumpText(); }
  const box = $('logbox');
  if (box && L.follow) box.scrollTop = box.scrollHeight;
}

function pickLog(name, render) {
  Object.assign(S.log, { name, offset: null, lines: [], tail: '', follow: true, newBelow: 0, tags: {}, query: '' });
  if (name) delete S.log.errors[name];
  if (render) renderPage();
  if (name) readLog();
}

async function readLog() {
  const L = S.log;
  const name = L.name;
  let r;
  try { r = await daemon({ cmd: 'log_read', name, offset: L.offset }); } catch { return; }
  if (name !== L.name) return;
  if (L.offset !== null && r.offset < L.offset) { L.lines = []; L.tail = ''; } // the file was cleared or rotated
  L.offset = r.offset;
  if (!r.text) return;
  const parts = (L.tail + r.text).split('\n');
  L.tail = redraw(parts.pop());
  const fresh = parts.map((t) => redraw(t).replace(/\r$/, ''));
  if (!L.follow) L.newBelow += fresh.length;
  L.lines = L.lines.concat(fresh).slice(-2000);
  const box = $('logbox');
  if (box) fillLog(box);
  syncFollow();
}

function fillLog(box) {
  const L = S.log;
  const all = L.tail ? L.lines.concat(L.tail.replace(/\r$/, '')) : L.lines;
  // The level and the search belong to the Logs page; a site's log panel shows every line.
  const page = S.page === 'logs';
  const q = page ? L.query.trim().toLowerCase() : '';
  const level = page ? L.level : 'all';
  const shown = all.filter((t) => (level === 'all' || (level === 'warn' ? lineLevel(t) : lineLevel(t) === 'error')) && (!q || t.toLowerCase().includes(q)));
  box.replaceChildren(...(shown.length ? shown.map((t) => {
    // concurrently's [name] prefixes, one color each, in the order they first appear.
    const tag = /^\[[A-Za-z][\w:.-]{0,23}\] /.exec(t)?.[0];
    if (tag && !(tag in L.tags)) L.tags[tag] = Object.keys(L.tags).length % 2 ? 'b' : 'a';
    return h('div.dv-ln' + (lineLevel(t) ? '.' + lineLevel(t) : ''), {}, tag ? [h('span.lg-tag.' + L.tags[tag], {}, tag), t.slice(tag.length)] : t);
  }) : [h('div.dv-sub.dv-empty', {}, all.length ? 'No lines match this filter.' : L.name + ' is empty.')]));
  if ($('lg-count')) $('lg-count').textContent = q ? shown.length + (shown.length === 1 ? ' match' : ' matches') : '';
  if (L.follow || S.logPanel) box.scrollTop = box.scrollHeight;
}

/** What the daemon's automatic check found (D-028): a newer devid and the binary changes. */
function autoUpdates(st) {
  const u = st?.updates;
  if (!u?.checked_at) return null;
  const changes = u.changes || [];
  return { devid: u.devid || null, changes, count: changes.length + (u.devid ? 1 : 0), checkedAt: u.checked_at };
}

/** A switch row; `label` and `sub` describe it, `change` gets the new state. */
function switchRow(id, label, sub, on, change) {
  return row(h('label.dv-grow', { for: id }, h('div', {}, label), h('div.dv-sub', {}, sub)),
    h('input.dv-switch', { type: 'checkbox', role: 'switch', id, checked: on, disabled: !!S.busy, onchange: (e) => change(e.target.checked) }));
}

function settingsPage() {
  const st = S.status;
  const ports = portsText(st);
  const doc = S.doctor;
  const auto = autoUpdates(st);
  const minutes = st.update_check_minutes;
  return [
    // D-031: two settings, because the sites and the app start separately.
    h('h2.dv-h2', {}, 'Login'),
    card(
      switchRow('sw-autostart', 'Serve sites at login', 'nginx and PHP start in the background when you log in. Off: they wait until you press Start all.',
        st.autostart !== false, (on) => act(on ? 'Turning on…' : 'Turning off…', () => daemon({ cmd: 'autostart', enabled: on }),
          on ? 'Sites are served at login' : 'After login, nginx and PHP wait for Start all')),
      switchRow('sw-login', 'Open Devid at login', 'Shows the Devid icon in the menu bar. Your sites don\'t need it.',
        !!st.login_item, (on) => act(on ? 'Turning on…' : 'Turning off…', () => invoke('login_item', { enabled: on }),
          (r) => r.login_item ? 'Devid opens at login' : 'Devid no longer opens at login'))),
    h('h2.dv-h2', {}, 'Domains'),
    card(row(text('Top-level domain', 'Every site is served over HTTPS with a local certificate'), h('span.dv-mono.dv-strong', {}, '.' + st.tld)),
      row(text('Ports'), h('span.dv-sub.dv-mono', {}, ports))),
    h('h2.dv-h2', {}, 'Storage'),
    card(row(text('Data folder', h('span.dv-mono', {}, st.home)), btn('Show in Finder', () => openTarget({ what: 'finder', path: st.home })))),
    h('h2.dv-h2', {}, 'Updates'),
    card(devidItem(st, auto), runtimeItem(auto)),
    h('p.dv-sub.dv-note', {}, minutes
      ? 'Devid checks every ' + plural(minutes, 'minute') + (auto ? ', last ' + Math.max(0, Math.round((Date.now() / 1000 - auto.checkedAt) / 60)) + ' min ago' : '') +
        '. The menu-bar icon shows a dot when something is new.'
      : 'Automatic checks are off (update_check_minutes = 0 in config.toml).'),
    h('h2.dv-h2', {}, 'Health check'),
    card(
      row(text('Health check', doc?.error || (doc?.checks ? (doc.checks.filter((c) => !c.ok).length ? plural(doc.checks.filter((c) => !c.ok).length, 'problem') + ' found' : 'All good') :
        'Checks services, ports, DNS, certificates and php-fpm.')),
      busyBtn('doctor', doc?.checks ? 'Check again' : 'Check now', () => act('Checking…', async () => {
        try { S.doctor = { checks: await invoke('doctor') }; } catch (e) { S.doctor = { error: String(e) }; }
      }, null, 'doctor'))),
      (doc?.checks || []).map((c) => h('div.dv-row', {},
        h('span.dv-chip.' + (c.ok ? 'ok' : 'bad'), {}, c.ok ? 'OK' : 'Problem'),
        text(c.what, c.fix ? 'Fix: ' + c.fix : '')))),
    h('h2.dv-h2', {}, 'Setup'),
    card(
      row(text('Setup', 'Runs devid setup in Terminal: system pieces, PHP and the web server'),
        btn('Run setup…', () => act('Opening Terminal…', () => invoke('in_terminal', { action: 'setup' }), (m) => m))),
      row(text('Uninstall Devid', 'Sites stop and the system setup is undone. Your data in ~/.devid is kept.'),
        btn('Uninstall…', async () => {
          if (await ask('Uninstall Devid?', 'Terminal opens and runs devid uninstall. Sites stop and the system setup is undone (sudo). Your data in ~/.devid is kept.', 'Uninstall', true))
            act('Opening Terminal…', () => invoke('in_terminal', { action: 'uninstall' }), (m) => m);
        }, 'danger-text'))),
  ];
}

// ---------- Settings: updates with their progress (the v0.12.x "Settings · Update progress" board) ----------

const sleep = (ms) => new Promise((r) => setTimeout(r, ms));
const isPhp = (c) => c.what.startsWith('php ');
/** A change as its step is named: "PHP 8.4.27 → 8.4.28", "nginx 1.30.6 → 1.30.7". */
const changeName = (c) => (isPhp(c) ? 'PHP' : c.what) + ' ' + c.from + ' → ' + c.to;
/** "PHP 8.4.28", "nginx 1.30.7". */
const changeShort = (c) => (isPhp(c) ? 'PHP ' + c.to : c.what + ' ' + c.to);
const sizeOf = (list) => list.reduce((n, c) => n + (c.size || 0), 0);
const are = (list) => list.join(', ') + (list.length > 1 ? ' are' : ' is');
const were = (list) => list.join(', ') + (list.length > 1 ? ' were' : ' was');

/** Keeps the bytes of the last few seconds, for the speed. */
function sample(u, bytes) {
  const now = Date.now();
  u.samples = (u.samples || []).filter((x) => now - x.t < 4000).concat({ t: now, bytes });
}

/** " · 3.1 MB/s · about 12 s left" from the samples, once a second of them shows a speed. */
function pace(u, left) {
  const [a, b] = [u.samples?.[0], u.samples?.[u.samples.length - 1]];
  const rate = a && b.t - a.t > 900 ? (b.bytes - a.bytes) * 1000 / (b.t - a.t) : 0;
  if (rate <= 0) return '';
  const s = left / rate;
  return ' · ' + humanSize(rate) + '/s · about ' + (s < 60 ? Math.max(1, Math.ceil(s)) + ' s' : Math.ceil(s / 60) + ' min') + ' left';
}

/**
 * Runs `job` while `progress` is read into `u` every 400 ms. The rest of the app waits, as with
 * act(). Resolves to [reply, error].
 */
async function follow(u, key, job, progress) {
  S.busy = 'Updating…';
  S.busyAt = key;
  renderChrome();
  renderPage();
  let done = false;
  const settled = job.then((r) => [r, ''], (e) => [null, String(e)]).finally(() => { done = true; });
  while (!done) {
    await sleep(400);
    const p = await progress().catch(() => null);
    if (p && !done) { Object.assign(u, p); renderChrome(); renderPage(); }
  }
  const result = await settled;
  S.busy = '';
  S.busyAt = '';
  await refresh(true);
  return result;
}

/** One update item: icon, title, chip, what it says, its percent and buttons; while it runs or after
 * it failed, the bar, a line under it, the steps and the error; then "Updated just now" for a while. */
function updateItem(o) {
  const running = o.state === 'running', failed = o.state === 'failed';
  return h('div.up-item' + (running ? '.is-running' : failed ? '.is-failed' : ''), {},
    h('div.up-head', {}, o.icon,
      h('div.dv-grow', {}, h('div.up-title', {}, o.title, o.chip ? h('span.dv-chip.' + o.chipCls, {}, o.chip) : null),
        h('div.dv-sub', { 'aria-live': 'polite' }, o.sub)),
      running || failed ? h('span.up-pct' + (failed ? '.bad' : ''), { 'aria-hidden': 'true' }, o.pct + '%') : null,
      running ? null : h('div.up-actions', {}, o.actions)),
    running || failed ? h('div.up-body', {},
      h('progress.up-bar' + (failed ? '.bad' : ''), { max: '100', value: o.pct, 'aria-label': o.title + ' update, ' + o.pct + '%' }),
      h('div.up-meta', {}, h('span', {}, o.metaLeft), h('span', {}, o.metaRight)),
      h('ol.up-steps', { 'aria-label': 'Steps' }, o.steps.map((x) => h('li.up-step.is-' + x.cls, {},
        h('span.up-sico', { 'aria-hidden': 'true' }, { done: '✓', bad: '✕' }[x.cls] || ''), h('span.up-sname', {}, x.name), h('span.up-sstate', {}, x.state)))),
      failed ? h('div.up-err', { role: 'alert' }, h('div', {}, h('b', {}, o.errTitle), ' ' + o.error), h('div.up-errmsg', {}, o.errMsg)) : null) : null,
    o.done ? h('div.up-done' + (o.done === 'fading' ? '.is-fading' : ''), { role: 'status' }, h('span.up-sico', { 'aria-hidden': 'true' }, '✓'), 'Updated just now') : null,
    o.wait ? h('div.dv-sub.up-wait', {}, o.wait) : null);
}

const LOGO = '<svg viewBox="0 0 100 100"><rect width="100" height="100" rx="22" fill="#16151F"/><path d="M37 46 V37 A13 13 0 0 1 63 37 V46" fill="none" stroke="#34C77B" stroke-width="10" stroke-linecap="round"/><rect x="24" y="44" width="52" height="38" rx="9" fill="#F4F4FA"/><path d="M37 56 L45 63 L37 70" fill="none" stroke="#16151F" stroke-width="7" stroke-linecap="round" stroke-linejoin="round"/><path d="M52 70 H62" fill="none" stroke="#16151F" stroke-width="7" stroke-linecap="round"/></svg>';
function logo() {
  const t = document.createElement('template');
  t.innerHTML = LOGO; // constant markup, never data
  const svg = t.content.firstElementChild;
  svg.setAttribute('class', 'up-icon');
  svg.setAttribute('aria-hidden', 'true');
  return svg;
}

/** The newer devid, from a check in this window or the daemon's own (D-028); null if none or unknown. */
const devidTarget = (auto) => S.devid ? (S.devid.available ? (/devid (\S+) is available/.exec(S.devid.text) || [])[1] || null : null) : auto?.devid || null;

/** Devid's steps take fixed parts of the bar; within a download its part follows the bytes. */
function devidPercent(u) {
  const part = (from, to) => from + Math.floor((to - from) * (u.size ? Math.min(1, (u.got || 0) / u.size) : 0));
  return { devid: part(0, 45), install: 50, app: part(55, 90), open: 100 }[u.step] ?? 0;
}

function devidItem(st, auto) {
  const u = S.upd.dv;
  const to = u?.to || devidTarget(auto);
  const state = u?.state || (S.busyAt === 'devid-check' ? 'checking' : '');
  const running = state === 'running', failed = state === 'failed';
  // Only this window's own check says "newest": the daemon's leaves devid empty also when its check failed.
  const latest = !to && S.devid && !S.devid.available;
  const step = failed ? u.failedStep : u?.step;
  const line = step === 'app' || step === 'open' ? 1 : 0; // devid and install are the first line
  const complete = u?.size && u.got >= u.size;
  const bytes = () => humanSize(u.got || 0) + (u.size ? ' of ' + humanSize(u.size) : '');
  const steps = !(running || failed) ? [] : [['devid ' + to, { devid: complete ? 'Checking sha256…' : bytes(), install: 'Installing · the daemon restarts' }],
    ['Devid.app ' + to, { app: complete ? 'Replacing Devid.app' : bytes(), open: 'Installed' }]].map(([name, now], i) =>
    i < line || step === 'open' ? { name, cls: 'done', state: 'Installed' }
      : i > line ? { name, cls: failed ? 'skip' : 'pending', state: failed ? 'Not updated' : 'Waiting' }
      : failed ? { name, cls: 'bad', state: 'Failed' } : { name, cls: 'now', state: now[step] || 'Starting…' });
  if (running && (step === 'devid' || step === 'app') && !complete) sample(u, u.got || 0);
  return updateItem({
    state, icon: logo(), title: 'Devid',
    chip: running || failed || to ? st.version + ' → ' + to : latest ? 'Up to date' : st.version,
    chipCls: failed ? 'bad' : latest ? 'ok' : to ? 'accent' : 'muted',
    sub: state === 'checking' ? 'Checking for updates…'
      : running ? 'Updating Devid. This window reopens in the new version when it is done.'
      : failed ? 'The update stopped.'
      : to ? 'devid and Devid.app ' + to : latest ? 'You have the newest version.' : 'Updates devid and this app',
    pct: u ? devidPercent(u) : 0,
    metaLeft: step === 'open' ? 'Opening Devid ' + to + '…' : (failed ? 'Stopped at step ' : 'Step ') + (line + 1) + ' of 2',
    metaRight: !running || step === 'open' ? '' : step === 'install' ? 'Sites pause for a few seconds'
      : complete ? 'Verifying the download' : bytes() + (u.size ? pace(u, u.size - (u.got || 0)) : ''),
    steps,
    errTitle: (step === 'app' ? 'Devid.app ' : 'devid ') + to + ' failed:',
    error: u?.error || '',
    errMsg: step === 'devid' ? 'Devid ' + st.version + ' is still installed and running.'
      : step === 'app' ? 'devid ' + to + ' is installed; Devid.app was not updated. Try again.'
      : 'Try again, or run devid setup in Terminal to see each step.',
    actions: failed ? [busyBtn('devid-update', 'Try again', () => updateDevid(true), 'primary')] : [
      busyBtn('devid-check', 'Check for updates', () => act('Checking…', async () => {
        const text = await invoke('devid_check');
        S.devid = { text, available: / is available /.test(text) };
      }, null, 'devid-check')),
      to ? busyBtn('devid-update', 'Update Devid…', () => updateDevid(), 'primary') : null],
    wait: S.upd.rt?.state === 'running' && (to || failed) ? 'Available after PHP and the web server finish updating.' : '',
  });
}

function runtimeItem(auto) {
  const u = S.upd.rt;
  const known = S.updates || (auto ? { text: '', changes: auto.changes } : null);
  const changes = known?.changes || [];
  const state = u?.state === 'done' ? '' : u?.state || (S.busyAt === 'updates-check' ? 'checking' : '');
  const running = state === 'running', failed = state === 'failed';
  const items = running || failed ? u.items : changes;
  const shorts = items.map(changeShort);
  const restarts = (c) => isPhp(c) ? 'php-fpm ' + c.what.slice(4) : ['nginx', 'caddy'].includes(c.what) ? c.what : '';
  const now = (c) => ({
    downloading: u.size ? humanSize(u.got || 0) + ' of ' + humanSize(u.size) : humanSize(u.got || 0),
    checking: 'Checking sha256…',
    installing: 'Installing' + (restarts(c) ? ' · ' + restarts(c) + ' restarts' : '…'),
    restarting: 'Restarting php-fpm ' + c.what.split(' ').pop(),
  })[u.phase] || 'Starting…';
  const steps = !(running || failed) ? [] : items.map((c, i) => {
    const name = changeName(c);
    if (i < u.index) return { name, cls: 'done', state: c.to.startsWith('removed') ? 'Removed' : 'Installed' };
    if (failed) return i === u.index && u.index >= 0 ? { name, cls: 'bad', state: 'Failed' } : { name, cls: 'skip', state: 'Not updated' };
    if (i === u.index) return { name, cls: 'now', state: now(c) };
    return { name, cls: 'pending', state: 'Waiting' + (c.size ? ' · ' + humanSize(c.size) : '') };
  });
  const bytes = running ? (u.done_bytes || 0) + (u.got || 0) : 0;
  if (running && u.phase === 'downloading') sample(u, bytes);
  const installed = failed ? items.slice(0, Math.max(0, u.index)).map(changeShort) : [];
  const skipped = failed ? items.slice(Math.max(0, u.index) + 1).map(changeShort) : [];
  const recent = u?.state === 'done' ? Date.now() - u.doneAt : Infinity;
  return updateItem({
    state, icon: h('span.up-icon.rt', { 'aria-hidden': 'true' }, icon('php')), title: 'PHP, web server and extensions',
    chip: running ? Math.max(0, u.index) + ' of ' + items.length + ' installed' : failed ? 'Failed'
      : changes.length ? plural(changes.length, 'update') : known ? 'Up to date' : '',
    chipCls: running || changes.length ? 'accent' : failed ? 'bad' : 'ok',
    sub: state === 'checking' ? 'Checking for updates…'
      : running ? 'Updating one at a time. Each restart pauses the sites that use it for a second or two.'
      : failed ? 'The update stopped. Nothing that was already installed is undone.'
      : u?.state === 'done' ? 'Last updated today at ' + new Date(u.doneAt).toLocaleTimeString('en-US', { hour: 'numeric', minute: '2-digit' }) + ' · ' + u.items.map(changeShort).join(', ')
      : changes.length ? shorts.join(', ') + (sizeOf(changes) ? ' · ' + humanSize(sizeOf(changes)) + ' download' : '') + (known.unknown?.length ? ' · version unknown, skipped: ' + known.unknown.join(', ') : '')
      : known?.text || 'Newer builds from Devid\'s release',
    pct: u?.percent || 0,
    metaLeft: (failed ? 'Stopped at step ' : 'Step ') + (Math.max(0, u?.index ?? 0) + 1) + ' of ' + items.length,
    metaRight: !running ? '' : u.phase === 'downloading' && u.total_bytes ? humanSize(bytes) + ' of ' + humanSize(u.total_bytes) + pace(u, u.total_bytes - bytes)
      : u.phase === 'checking' ? 'Verifying the download' : '',
    steps,
    errTitle: items[u?.index] ? changeShort(items[u.index]) + ' failed:' : 'The update failed:',
    error: u?.error || '',
    errMsg: u?.index < 0 ? 'Nothing was installed.'
      : (installed.length ? are(installed) + ' installed. ' : '') + (skipped.length ? were(skipped) + ' not updated. ' : '') + 'Try again picks up from ' + changeShort(items[u?.index] || items[0] || { what: '', to: '' }) + '.',
    actions: failed ? [busyBtn('updates-apply', 'Try again', () => updateBinaries(true), 'primary')] : [
      busyBtn('updates-check', 'Check for updates', () => act('Checking…', async () => {
        S.updates = updateSummary(await daemon({ cmd: 'update', check: true }), true);
        S.upd.rt = null;
      }, null, 'updates-check')),
      changes.length ? busyBtn('updates-apply', 'Update…', () => updateBinaries(), 'primary') : null],
    done: recent < 4600 ? 'shown' : recent < 5000 ? 'fading' : '',
    wait: S.upd.dv?.state === 'running' && (changes.length || failed) ? 'Available after the Devid update finishes.' : '',
  });
}

/**
 * Updates devid, its daemon and this app in the background (sha256-checked), with each step and its
 * download shown, then opens the new version's window on Settings. Sites are down for a few seconds
 * while the daemon restarts. Only a system setup step that needs the password goes to Terminal.
 */
async function updateDevid(again = false) {
  const to = devidTarget(autoUpdates(S.status));
  if (!again && !await ask('Update Devid' + (to ? ' to ' + to : '') + '?', 'Downloads devid and Devid.app (sha256-checked) and restarts the daemon, ' +
    'so your sites pause for a few seconds; this window then reopens in the new version.', 'Update'))
    return;
  const u = S.upd.dv = { state: 'running', to, step: 'devid', got: 0, size: 0 };
  const [reply, error] = await follow(u, 'devid-update', invoke('self_update'), async () => {
    const p = await invoke('self_update_progress');
    if (p && p.step !== u.step) u.samples = [];
    return p && { ...p, got: p.got || 0, size: p.size || 0 };
  });
  if (reply?.installed) {
    Object.assign(u, { step: 'open', to: reply.installed });
    S.busy = 'Opening…'; // until this window is replaced by the new version's
    renderChrome();
    renderPage();
    toast('Devid ' + reply.installed + ' is installed; opening it…');
    setTimeout(() => invoke('relaunch', { page: 'settings' }), 600);
  } else if (reply) {
    S.upd.dv = null;
    S.devid = { text: reply.text, available: false };
    toast(reply.text);
  } else if (/needs your password/.test(error)) {
    S.upd.dv = null;
    if (await ask('Finish in Terminal?', error.replace(/`/g, '') + '.', 'Open Terminal'))
      act('Opening Terminal…', () => invoke('in_terminal', { action: 'setup' }), (m) => m);
  } else {
    Object.assign(u, { state: 'failed', failedStep: u.step, error });
    toast(error, true);
  }
  renderChrome();
  renderPage();
}

/** Installs what is newer in the release, one at a time, following the daemon's progress. */
async function updateBinaries(again = false) {
  const failed = S.upd.rt?.state === 'failed' ? S.upd.rt : null;
  const changes = (failed ? failed.items.slice(Math.max(0, failed.index)) : (S.updates || autoUpdates(S.status))?.changes) || [];
  const kinds = [...new Set(changes.map((c) => isPhp(c) ? 'PHP' : c.what.includes(' for php ') ? 'extensions' : c.what))];
  if (!again && !await ask('Update ' + andList(kinds) + '?', 'Downloads ' + changes.map(changeShort).join(', ') +
    (sizeOf(changes) ? ' (' + humanSize(sizeOf(changes)) + ', sha256-checked)' : ' (sha256-checked)') + ' and installs them one at a time.', 'Update'))
    return;
  const u = S.upd.rt = { state: 'running', items: changes, index: 0, phase: 'starting', percent: 0, got: 0, size: 0, done_bytes: 0, total_bytes: sizeOf(changes) };
  // Only this update's progress: the last one's end stays in the daemon until this one starts.
  const progress = async () => { const p = (await daemon({ cmd: 'update_progress' })).progress; return p?.state === 'running' ? p : null; };
  const [reply, error] = await follow(u, 'updates-apply', daemon({ cmd: 'update', check: false }), progress);
  const end = (await daemon({ cmd: 'update_progress' }).catch(() => null))?.progress;
  if (reply) {
    S.updates = updateSummary(reply, false);
    Object.assign(u, { state: 'done', percent: 100, doneAt: Date.now(), items: reply.changes || u.items });
    toast(S.updates.text);
    setTimeout(renderPage, 4700);
    setTimeout(renderPage, 5100);
  } else {
    Object.assign(u, end?.state === 'failed' ? end : { index: -1 }, { state: 'failed', error });
    toast(error, true);
  }
  renderChrome();
  renderPage();
}

/** The TUI's summary of an `update` reply (crates/cli/src/tui/mod.rs), with its changes. */
function updateSummary(v, check) {
  const changes = v.changes || [];
  const unknown = v.unknown || [];
  let text = !changes.length ? (unknown.length ? 'No updates found' : 'Everything is up to date (release ' + v.release + ')')
    : (check ? 'Available: ' : 'Updated: ') + changes.map((c) => c.what + ' ' + c.from + '→' + c.to).join(', ');
  if (unknown.length) text += ' · version unknown, skipped: ' + unknown.join(', ') + ' (try again)';
  return { text, changes: check ? changes : [], unknown };
}

// ---------- Extensions dialog (Phase 6 catalog) ----------

async function openExtensions(version) {
  S.ext = { version, tab: 'installed', list: null, catalog: null, catalogError: '', selected: new Set(), ask: null, job: '', result: null, removing: '' };
  await loadExtensions();
}

async function loadExtensions() {
  const x = S.ext;
  try { x.list = await daemon({ cmd: 'php_ext_list', version: x.version }); } catch (e) { S.ext = null; return toast(String(e), true); }
  try { x.catalog = await daemon({ cmd: 'php_ext_catalog', version: x.version }); x.catalogError = ''; } catch (e) { x.catalogError = String(e); }
  renderExt();
}

function renderExt() {
  const x = S.ext;
  if (!x) return;
  const v = x.version;
  const locked = !!x.job || !!x.removing;
  const tabs = h('div.dv-seg', { role: 'group', 'aria-label': 'Extension view' }, [['installed', 'Installed'], ['catalog', 'From the catalog']].map(([k, l]) =>
    h('button', { type: 'button', 'aria-pressed': String(x.tab === k), 'data-focus': x.tab === k && !x.ask, onclick: () => { x.tab = k; x.ask = null; renderExt(); } }, l)));
  const body = x.tab === 'installed' ? extInstalled(x, locked) : extCatalog(x, locked);
  showDialog(h('div.dv-dialog.wide', { role: 'dialog', 'aria-modal': 'true', 'aria-labelledby': 'ext-t' },
    h('h2', { id: 'ext-t' }, 'PHP ' + v + ' extensions'),
    h('p.dv-sub', {}, 'Built-in extensions are always on. Add more from the Devid catalog.'),
    h('div.dv-tabs-row', {}, tabs),
    body,
    h('p.dv-sub', {}, 'Built for PHP ' + v + ' on Intel Macs, from Devid\'s catalog only. Nothing is compiled on your Mac.'),
    h('div.dv-dialog-btns', {}, h('button.dv-btn.primary', { type: 'button', onclick: closeDialog, disabled: locked }, 'Done'))),
  () => { if (!locked) closeDialog(); });
}

function extInstalled(x, locked) {
  const v = x.version;
  const shipped = Object.entries(x.list.shipped || {});
  const catalog = x.list.catalog || [];
  return [
    h('div.dv-section', {}, 'Built-in ', h('span.dv-sub.dv-light', {}, '· compiled into PHP ' + v + ', always on')),
    h('div.dv-card', {}, h('div.dv-chips', {}, (x.list.bundled || []).map((n) => h('span.dv-chip.muted.dv-mono', {}, n)))),
    h('div.dv-section', {}, 'Shipped with PHP'),
    card(shipped.map(([name, on]) => row(text(h('span.dv-mono', {}, name), EXT_INFO[name] || ''),
      h('input.dv-switch', { type: 'checkbox', role: 'switch', 'aria-label': name, checked: on, disabled: locked,
        onchange: (e) => extRun('Turning ' + name + ' ' + (e.target.checked ? 'on' : 'off') + '…',
          () => daemon({ cmd: 'php_ext', version: v, ext: name, enabled: e.target.checked }),
          name + ' ' + (e.target.checked ? 'on' : 'off') + ' for PHP ' + v + ' (php-fpm restarted)') })))),
    h('div.dv-section', {}, 'From the catalog'),
    card(catalog.length ? catalog.map((e) => {
      if (x.removing === e.name) return row(h('div.dv-status-line.dv-grow', { role: 'status' }, 'Removing ' + e.name + '…'));
      if (x.ask?.remove === e.name)
        return h('div.dv-row.is-asking', { role: 'alertdialog', 'aria-labelledby': 'rm-t' },
          text(h('span.dv-strong', { id: 'rm-t' }, 'Remove ' + e.name + ' ' + e.version + ' from PHP ' + v + '?'), 'php-fpm ' + v + ' restarts; code that uses it stops working.'),
          h('button.dv-btn', { type: 'button', onclick: () => { x.ask = null; renderExt(); } }, 'Cancel'),
          h('button.dv-btn.danger', { type: 'button', 'data-focus': true, onclick: () => {
            x.ask = null; x.removing = e.name;
            extRun('', () => daemon({ cmd: 'php_ext_remove', version: v, name: e.name }), e.name + ' removed from PHP ' + v + ' (php-fpm restarted)');
          } }, 'Remove'));
      return row(text([h('span.dv-mono', {}, e.name), ' ', h('span.dv-sub.dv-mono', {}, e.version)], EXT_INFO[e.name] || ''),
        h('button.dv-btn.danger-text', { type: 'button', disabled: locked, onclick: () => { x.ask = { remove: e.name }; renderExt(); } }, 'Remove'));
    }) : h('div.dv-empty.dv-sub', {}, 'No catalog extensions installed yet.')),
  ];
}

function extCatalog(x, locked) {
  const v = x.version;
  if (x.catalogError) return [h('div.dv-banner', { role: 'status' }, text(h('span.dv-strong', {}, 'The catalog is not available'), x.catalogError))];
  const list = x.catalog.extensions || [];
  const chosen = list.filter((e) => x.selected.has(e.name));
  const names = chosen.map((e) => e.name + ' ' + e.version);
  const size = chosen.reduce((n, e) => n + (e.size || 0), 0);
  const bar = x.ask === 'install'
    ? h('div.dv-row.is-asking.go', { role: 'alertdialog', 'aria-labelledby': 'inst-t' },
      text(h('span.dv-strong', { id: 'inst-t' }, 'Install ' + andList(names) + ' for PHP ' + v + '?'),
        'About ' + humanSize(size) + ' from Devid\'s releases (sha256-checked). php-fpm ' + v + ' restarts once, so its sites pause for a second or two.'),
      h('button.dv-btn', { type: 'button', onclick: () => { x.ask = null; renderExt(); } }, 'Cancel'),
      h('button.dv-btn.primary', { type: 'button', 'data-focus': true, onclick: () => installExtensions(chosen.map((e) => e.name)) }, 'Install'))
    : h('div.dv-bar', {},
      h('div.dv-grow', {}, x.job ? h('div.dv-status-line', { role: 'status' }, x.job) :
        x.result ? h('div.dv-status-line', { role: 'status' }, x.result.text) : null),
      h('button.dv-btn.primary', { type: 'button', disabled: locked || !chosen.length, onclick: () => { x.ask = 'install'; renderExt(); } },
        'Install selected (' + chosen.length + ')'));
  return [
    h('div.dv-card', {}, list.length ? list.map((e) => {
      const fresh = e.installed !== e.version; // not installed, or an update
      return h('label.dv-row', {},
        fresh ? h('input.dv-check', { type: 'checkbox', 'aria-label': 'Select ' + e.name, checked: x.selected.has(e.name), disabled: locked,
          onchange: (ev) => { ev.target.checked ? x.selected.add(e.name) : x.selected.delete(e.name); x.ask = null; renderExt(); } }) : null,
        text([h('span.dv-mono', {}, e.name), ' ', h('span.dv-sub.dv-mono', {}, e.version), h('span.dv-sub', {}, ' · ' + e.license + ' · ' + humanSize(e.size || 0))],
          EXT_INFO[e.name] || ''),
        e.installed === e.version ? h('span.dv-chip.ok', {}, '✓ Installed') : e.installed ? h('span.dv-chip.muted', {}, 'Installed ' + e.installed) : null);
    }) : h('div.dv-empty.dv-sub', {}, 'Nothing for PHP ' + v + ' in this release.')),
    bar,
  ];
}

/** An action inside the Extensions dialog; its result goes to a toast, then the dialog reloads. */
async function extRun(label, run, done) {
  const x = S.ext;
  if (label) { x.job = label; renderExt(); }
  try { await run(); toast(done); } catch (e) { toast(String(e), true); }
  if (S.ext !== x) return;
  x.job = ''; x.removing = '';
  await loadExtensions();
}

/** Installs with the CLI's progress line, polling the daemon every 200 ms (as the TUI does). */
async function installExtensions(names) {
  const x = S.ext;
  x.ask = null; x.result = null; x.job = 'Starting…';
  renderExt();
  const install = daemon({ cmd: 'php_ext_install', version: x.version, names });
  let finished = false;
  install.catch(() => {}).finally(() => { finished = true; }); // the result is read below
  while (!finished) {
    await new Promise((r) => setTimeout(r, 200));
    const p = await daemon({ cmd: 'php_ext_progress' }).catch(() => null);
    if (!finished && p?.progress && S.ext === x) { x.job = progressText(p.progress); renderExt(); }
  }
  try {
    x.result = installSummary(await install);
    toast(x.result.text, x.result.failed);
  } catch (e) {
    x.result = { text: String(e), failed: true };
    toast(String(e), true);
  }
  x.selected.clear();
  x.job = '';
  if (S.ext === x) await loadExtensions();
}

/** `ext_progress_text` in crates/cli/src/main.rs. */
function progressText(p) {
  const n = p.index + '/' + p.total;
  switch (p.phase) {
    case 'downloading': return 'Installing ' + n + ' ' + p.name + '… ' + p.percent + '%';
    case 'checking': return 'Checking ' + n + ' ' + p.name + '… ' + p.percent + '%';
    case 'restarting': return 'Restarting php-fpm ' + p.version + '… ' + p.percent + '%';
    default: return 'Starting… ' + p.percent + '%';
  }
}

/** `ext_install_summary` in crates/cli/src/main.rs. */
function installSummary(reply) {
  const results = reply.results || [];
  const ok = results.filter((r) => r.ok).map((r) => r.name + ' ' + (r.installed || ''));
  const failed = results.filter((r) => !r.ok).map((r) => r.name + ' (' + String(r.error || '?').split('\n')[0] + ')');
  const parts = [];
  if (ok.length) parts.push('Installed ' + ok.join(', '));
  if (reply.restarted) parts.push('php-fpm ' + reply.version + ' restarted');
  if (failed.length) parts.push((parts.length ? 'failed: ' : 'Failed: ') + failed.join(', '));
  return { text: parts.join(' · '), failed: failed.length > 0 };
}

// ---------- chrome and refresh ----------

let chromeKey = '';
function renderChrome() {
  const st = S.status;
  // Redraw only on a change, so keyboard focus in the sidebar survives the polling.
  const cardBusy = S.busyAt === 'card' ? S.busy : '';
  const heading = PAGES[S.page].heading?.() || PAGES[S.page].title;
  const upd = [S.upd.dv, S.upd.rt].find((u) => u?.state === 'running');
  const pct = upd && (upd === S.upd.dv ? devidPercent(upd) : upd.percent || 0);
  const key = JSON.stringify([S.page, S.site, heading, PAGES[S.page].sub(), !!S.busy, cardBusy, autoUpdates(st)?.count, pct,
    st && [st.sites, st.default_php, st.server, st.server_status, st.php.map((p) => p.status)]]);
  if (key === chromeKey) return;
  chromeKey = key;
  $('nav').replaceChildren(...Object.entries(PAGES).map(([key, p]) =>
    h('button.dv-nav' + (key === S.page ? '.is-active' : ''), { type: 'button', 'aria-current': key === S.page ? 'page' : null,
      onclick: () => go(key) }, icon(key), h('span.dv-grow', {}, p.title),
    key === 'sites' && st ? h('span.dv-chip', {}, String(st.sites)) : key === 'php' && st ? h('span.dv-chip', {}, st.default_php) :
      key === 'settings' && upd ? h('span.dv-navpct', { 'aria-label': 'Updating, ' + pct + '%' }, pct + '%') :
      key === 'settings' && autoUpdates(st)?.count ? h('span.dv-navdot', { title: 'Updates available', 'aria-label': 'Updates available' }) : null)));
  $('title').textContent = heading;
  $('lead').replaceChildren(...(S.page === 'sites' && S.site ? [h('button.dv-btn.ghost.icon', { type: 'button', 'aria-label': 'Back to Sites',
    title: 'Back to Sites', onclick: () => openSite(null) }, icon('back'))] : []));
  $('subtitle').textContent = PAGES[S.page].sub();
  $('tools').replaceChildren(...(PAGES[S.page].tools?.() || []));

  if (!st) {
    $('status-dot').className = 'dv-dot bad';
    $('status-server').textContent = 'Devid';
    $('status-line').textContent = 'Not running';
    $('status-btns').replaceChildren();
    $('status-more').replaceChildren();
    return;
  }
  const failed = st.php.filter((p) => p.status === 'failed').length + (st.server_status === 'failed' ? 1 : 0);
  const running = st.server_status === 'running';
  $('status-dot').className = 'dv-dot ' + (failed ? 'bad' : running ? 'ok' : 'warn');
  $('status-server').textContent = st.server;
  $('status-line').textContent = cardBusy || ((failed ? plural(failed, 'service') + ' failed' : capital(st.server_status)) + ' · PHP ' + st.default_php);
  $('status-more').replaceChildren(h('button.dv-btn.ghost.icon', { type: 'button', 'aria-label': 'Devid menu', 'aria-haspopup': 'menu',
    'aria-expanded': 'false', onclick: (e) => quickMenu(e.currentTarget) }, icon('more')));
  $('status-btns').replaceChildren(...(running
    ? [btn('Restart', () => restart(st.server)), btn('Stop all', stopAll)]
    : [btn('Start all', startAll, 'primary')]));
}

function renderPage() {
  if (menuOpen || (!$('overlay').hidden && !S.ext)) { S.dirty = true; return; }
  // Typing in a field: redraw once it loses focus, so a poll never undoes what is typed.
  if (document.activeElement?.matches('#page input.dv-input')) { S.dirty = true; return; }
  S.dirty = false;
  const page = $('page');
  page.parentElement.classList.toggle('is-panes', S.page === 'logs' && !!S.status);
  if (!S.status) {
    page.replaceChildren(h('div.dv-banner', { role: 'status' }, h('span.dv-dot.bad'),
      text(h('span.dv-strong', {}, 'Devid is not running'), S.error || '')));
    return;
  }
  const data = S.data[S.page];
  if (!data) return;
  const scroll = page.parentElement.scrollTop;
  const kept = ['lg-list', 'logbox'].map((id) => [id, $(id)?.scrollTop]);
  page.replaceChildren(...[versionBanner(), PAGES[S.page].render(data)].flat().filter(Boolean));
  page.parentElement.scrollTop = scroll;
  for (const [id, top] of kept) if (top != null && $(id)) $(id).scrollTop = top;
  const box = $('logbox');
  if (box && (S.log.follow || S.logPanel)) box.scrollTop = box.scrollHeight;
}

/** G4: the app and the daemon come from the same release; say what to do when they differ. */
function versionBanner() {
  const app = window.DEVID_APP_VERSION;
  const daemon = S.status?.version;
  if (!app || !daemon || app === daemon) return null;
  const older = (a, b) => a.split('.').map(Number).reduce((r, n, i) => r || n - (Number(b.split('.')[i]) || 0), 0) < 0;
  const [title, sub] = older(app, daemon)
    ? ['Devid ' + daemon + ' is installed', 'This window is still ' + app + '. Close it, and the menu-bar icon restarts as the new version.']
    : ['The Devid daemon is ' + daemon, 'This app is ' + app + '. Update the daemon with devid self-update in Terminal.'];
  return h('div.dv-banner.warn', { role: 'status' }, h('span.dv-dot.warn'), text(h('span.dv-strong', {}, title), sub));
}

function go(key) {
  if (S.page === key && !S.site) return;
  S.page = key;
  S.site = null;
  S.logPanel = null;
  closeMenu();
  $('page').parentElement.scrollTop = 0;
  renderChrome();
  $('page').replaceChildren();
  refresh(true);
}

async function refresh(force = false) {
  try {
    S.status = await daemon({ cmd: 'status' });
    S.error = null;
  } catch (e) {
    S.status = null;
    S.error = String(e); // the daemon client's own wording (D-026)
  }
  renderChrome();
  const key = S.page;
  if (S.status) {
    try {
      const data = await PAGES[key].load();
      const json = JSON.stringify(data);
      if (key !== S.page || (!force && json === S.keys[key])) return;
      S.keys[key] = json;
      S.data[key] = data;
      renderChrome(); // the title may come from the data (a site's detail)
    } catch (e) {
      toast(String(e), true);
    }
  }
  if (key === S.page) renderPage();
}

// Status every 2 s and the followed log every second, only while the window is visible.
// A closed window is destroyed with its web view (D-027).
let ticks = 0;
let timer = null;
function poll() {
  clearInterval(timer);
  timer = null;
  if (document.hidden) return;
  timer = setInterval(() => {
    ticks++;
    if (S.log.name && (S.page === 'logs' || (S.page === 'sites' && S.logPanel))) readLog();
    if (ticks % 2 === 0 && !S.busy) refresh();
  }, 1000);
  refresh(true);
}
document.addEventListener('visibilitychange', poll);
document.addEventListener('focusout', (e) => {
  // After the field's own change handler, which may have started an action.
  if (e.target.matches?.('#page input.dv-input')) setTimeout(() => { if (S.dirty && !S.busy) renderPage(); });
});
document.addEventListener('mousedown', (e) => { if (menuOpen && !menuOpen.m.contains(e.target) && !menuOpen.anchor.contains(e.target)) closeMenu(); });
document.addEventListener('keydown', (e) => {
  if (!menuOpen) return;
  const items = [...menuOpen.m.querySelectorAll('button:not(:disabled)')];
  const i = items.indexOf(document.activeElement);
  if (e.key === 'Escape') { menuOpen.anchor.focus(); closeMenu(); }
  else if (e.key === 'ArrowDown') { e.preventDefault(); items[(i + 1) % items.length]?.focus(); }
  else if (e.key === 'ArrowUp') { e.preventDefault(); items[(i - 1 + items.length) % items.length]?.focus(); }
});
// The menu bar's "Settings…" opens a new window on that page (main.rs sets this first).
if (PAGES[window.DEVID_START_PAGE]) S.page = window.DEVID_START_PAGE;
poll();
