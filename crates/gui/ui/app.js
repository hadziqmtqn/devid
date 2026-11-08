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
};

const S = {
  page: 'sites',
  status: null,
  error: null,
  data: {},
  keys: {},
  busy: '',
  log: { name: null, offset: null, lines: [], follow: true },
  updates: null, // {text, changes}
  devid: null, // {text, available}
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
function menu(anchor, entries, list = false) {
  closeMenu();
  const m = h('div.dv-menu' + (list ? '.is-list' : ''), { role: 'menu' },
    entries.filter(Boolean).map((e) =>
      e === '-' ? h('hr.dv-sep') :
      e.header ? h('div.dv-menu-h', {}, e.header) :
      h('button.dv-mi' + (e.danger ? '.danger' : ''), { type: 'button', role: 'checked' in e ? 'menuitemradio' : 'menuitem',
        'aria-checked': 'checked' in e ? String(e.checked) : null, disabled: !!S.busy, onclick: () => { closeMenu(); e.run(); } },
      'checked' in e ? h('span.dv-mark', { 'aria-hidden': 'true' }, e.checked ? '✓' : '') : null,
      e.label, e.kbd ? h('span.dv-kbd', {}, e.kbd) : null)));
  document.body.append(m);
  const r = anchor.getBoundingClientRect();
  const top = r.bottom + 4 + m.offsetHeight > innerHeight ? r.top - 4 - m.offsetHeight : r.bottom + 4;
  m.style.top = Math.max(8, top) + 'px';
  m.style.left = Math.max(8, Math.min(r.right - m.offsetWidth, innerWidth - m.offsetWidth - 8)) + 'px';
  anchor.setAttribute('aria-expanded', 'true');
  menuOpen = { m, anchor };
  (m.querySelector('[aria-checked="true"]') || m.querySelector('button'))?.focus();
}

function closeMenu() {
  if (!menuOpen) return;
  menuOpen.anchor.setAttribute('aria-expanded', 'false');
  menuOpen.m.remove();
  menuOpen = null;
  if (S.dirty) renderPage();
}

/** The prototype's select field; its list is the same menu as the ⋯ menus, not a native pop-up. */
function dropdown(label, value, options, pick) {
  return h('button.dv-select', { type: 'button', 'aria-label': label, 'aria-haspopup': 'menu', 'aria-expanded': 'false', disabled: !!S.busy,
    onclick: (e) => menu(e.currentTarget, options.map((o) => ({ label: o.label, checked: o.value === value,
      run: () => { if (o.value !== value) pick(o.value); } })), true) },
  h('span', {}, (options.find((o) => o.value === value) || { label: value }).label), icon('chevron'));
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
  // Names only: sizes change all the time and would redraw the page on every poll.
  logs: { title: 'Logs', sub: () => 'Log files in ' + (S.status?.home || '~/.devid') + '/log.',
    load: async () => ({ logs: (await daemon({ cmd: 'logs' })).logs.map((l) => ({ name: l.name })) }), render: logsPage },
  settings: { title: 'Settings', sub: () => 'Preferences for Devid on this Mac.', load: async () => ({}), render: settingsPage },
};

async function linkSite() {
  const path = await invoke('choose_folder', { purpose: 'link' }).catch((e) => toast(String(e), true));
  if (path) act('Linking…', () => daemon({ cmd: 'link', name: null, path }), 'Linked ' + path);
}

async function parkFolder() {
  const path = await invoke('choose_folder', { purpose: 'park' }).catch((e) => toast(String(e), true));
  if (path) act('Parking…', () => daemon({ cmd: 'park', path }), 'Parked ' + path);
}

function sitesPage(d) {
  const st = S.status;
  const installed = st.php.map((p) => p.version);
  const inside = (folder) => d.sites.filter((s) => !s.linked && s.path.startsWith(folder + '/')).length;
  const out = [];
  if (st.server_status === 'stopped')
    out.push(h('div.dv-banner.warn', { role: 'status' }, h('span.dv-dot.warn'),
      text(h('span.dv-strong', {}, 'Devid is stopped'), 'Your sites are not being served right now.'), btn('Start all', startAll, 'primary')));

  out.push(h('div.dv-head', {},
    h('div', {}, h('h2.dv-h2', {}, 'Parked folders ', h('span.dv-light', {}, '(' + d.parked.length + ')')),
      h('div.dv-sub', {}, 'Every folder inside a parked folder is served automatically at ', h('b', {}, 'folder-name'), '.' + st.tld)),
    btn([icon('plus'), 'Add folder'], parkFolder)));
  out.push(d.parked.length ? card(d.parked.map((folder) => {
    const n = inside(folder);
    return row(h('span.dv-avatar.folder', {}, icon('folder')),
      text(h('span.dv-mono', {}, folder), n ? plural(n, 'site') + ' served from here' : 'No sites yet'),
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

  // Removed parked sites (D-029) sit behind a filter on the Sites line, not in a section of their own.
  const ignored = d.ignored || [];
  if (!ignored.length) S.siteFilter = 'served';
  const removed = S.siteFilter === 'removed';
  out.push(h('div.dv-head', {},
    h('h2.dv-h2', {}, 'Sites ', h('span.dv-light', {}, '(' + (removed ? ignored.length : d.sites.length) + ')')),
    ignored.length ? dropdown('Show sites', removed ? 'removed' : 'served',
      [{ value: 'served', label: 'Served (' + d.sites.length + ')' }, { value: 'removed', label: 'Removed (' + ignored.length + ')' }],
      (v) => { S.siteFilter = v; renderPage(); }) : null));
  if (removed) {
    out.push(card(ignored.map((i) => row(h('span.dv-avatar', {}, i.name.charAt(0).toUpperCase()),
      text(i.name + '.' + st.tld, h('span.dv-mono', {}, i.path)),
      btn('Restore', () => act('Restoring…', () => daemon({ cmd: 'unignore', site: i.name }), 'Serving ' + i.name + ' again'))))));
    out.push(h('p.dv-sub.dv-note', {}, 'Removed sites are not served. Their folders are not touched.'));
    return out;
  }
  out.push(card(d.sites.length ? d.sites.map((s) => siteRow(s, installed, d.parked)) :
    h('div.dv-empty', {}, h('div.dv-strong', {}, 'No sites yet'), h('div.dv-sub', {}, 'Park a folder, or link one with the + button.'))));
  out.push(h('p.dv-sub.dv-note', {}, 'Each site picks its PHP version; a change reloads only that site.'));
  return out;
}

function siteRow(s, installed, parked) {
  const host = siteHost(s);
  const running = (s.commands || []).filter((c) => c.state === 'running').length;
  const versions = installed.includes(s.php) ? installed : [...installed, s.php];
  const pick = dropdown('PHP version for ' + host, s.php, versions.map((v) => ({ value: v, label: v })), (version) =>
    act('Switching ' + s.name + '…', () => daemon({ cmd: 'isolate', version, site: s.name, cwd: '/' }), s.name + ' now uses PHP ' + version));
  return row(
    h('span.dv-avatar', {}, s.name.charAt(0).toUpperCase()),
    h('div.dv-grow', {},
      h('div.dv-line', {},
        h('button.dv-host', { type: 'button', title: 'Details for ' + host, onclick: () => openSite(s.name) }, host),
        h('span.dv-chip.muted', {}, s.preset_label),
        s.mode === 'php' && s.isolated ? h('span.dv-chip.accent', { title: 'This site uses its own PHP version' }, 'Isolated') : null,
        s.mode !== 'php' || s.php_installed ? null : h('span.dv-chip.warn', {}, 'PHP ' + s.php + ' not installed')),
      h('div.dv-sub.dv-ellipsis', {}, h('span.dv-mono', {}, s.path), ' · ' + (s.linked ? 'Linked' : 'Parked'))),
    s.port ? h('span.dv-chip' + (s.port_busy ? '.warn' : '.muted'), { title: s.port_busy ? 'Port ' + s.port + ' is in use by ' + s.port_busy
      : 'Also served on http://127.0.0.1:' + s.port }, ':' + s.port + (s.port_busy ? ' busy' : '')) : null,
    running ? h('span.dv-chip.ok', {}, '● ' + running + ' running') : null,
    s.share?.url ? h('span.dv-chip.accent', { title: 'Shared at ' + s.share.url }, 'Shared') : null,
    s.mode === 'php' ? h('span.dv-label', {}, 'PHP', pick) : h('span.dv-sub.dv-nowrap', {}, s.mode === 'proxy' ? 'Proxy :' + s.upstream : 'Static'),
    h('button.dv-btn.ghost.icon', { type: 'button', title: 'Open in browser', 'aria-label': 'Open ' + host + ' in browser',
      onclick: () => openTarget({ what: 'site', name: s.name }) }, icon('open')),
    moreBtn('More actions for ' + host, () => [
      { header: host },
      { label: 'Details', run: () => openSite(s.name) },
      { label: 'Open in browser', run: () => openTarget({ what: 'site', name: s.name }) },
      { label: 'Open folder in Terminal', run: () => openTarget({ what: 'terminal', path: s.path }) },
      { label: 'Copy URL', run: () => act('Copying…', () => invoke('copy_url', { name: s.name }), (m) => m) },
      s.isolated && '-',
      s.isolated && { label: 'Use the default PHP again', kbd: S.status.default_php, run: () =>
        act('Switching ' + s.name + '…', () => daemon({ cmd: 'unisolate', site: s.name, cwd: '/' }), s.name + ' uses the default PHP again') },
      '-',
      s.linked ? { label: 'Unlink…', danger: true, run: async () => {
        if (await ask('Unlink ' + s.name + '?', 'The folder itself is not touched.', 'Unlink', true))
          act('Unlinking…', () => daemon({ cmd: 'unlink', name: s.name }), 'Unlinked ' + s.name);
      } } : { label: 'Remove site…', danger: true, run: async () => {
        const folder = parked.find((f) => s.path.startsWith(f + '/')) || '';
        if (await ask('Remove ' + host + '?', 'Devid stops serving it and ignores ' + s.path + ' inside ' + folder + '; the folder itself is not touched.', 'Remove', true))
          act('Removing…', () => daemon({ cmd: 'ignore', site: s.name }), (r) => 'Ignored ' + s.name + '; the folder is still in ' + r.parked);
      } },
    ]),
    h('button.dv-btn.ghost.icon', { type: 'button', title: 'Details', 'aria-label': 'Details for ' + host, onclick: () => openSite(s.name) }, icon('next')));
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
    s.mode === 'proxy' && row(label('app-up', 'Forward to', 'The port the dev server listens on.'), h('span.dv-sub.dv-mono', {}, 'localhost:'),
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
  const start = () => act('Starting ' + c.name + '…', () => req('command_start', c.name), 'Started ' + c.name);
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
    h('p.dv-sub.dv-note', {}, 'Static builds for Intel Macs (x86_64). Updates are in Settings.'),
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

function logsPage(d) {
  const L = S.log;
  if (!d.logs.some((l) => l.name === L.name)) {
    const php = 'php' + (S.status.default_php || '').replace('.', '') + '-fpm.log';
    pickLog(d.logs.some((l) => l.name === php) ? php : d.logs[0]?.name || null, false);
  }
  if (!L.name) return [card(h('div.dv-empty.dv-sub', {}, 'No log files yet.'))];
  const box = h('div.dv-log', { id: 'logbox', role: 'log', tabindex: '0', 'aria-label': L.name });
  fillLog(box);
  return [
    h('div.dv-logbar', {},
      h('div.dv-seg.dv-mono', { role: 'group', 'aria-label': 'Log file' }, d.logs.map((l) =>
        h('button', { type: 'button', 'aria-pressed': String(l.name === L.name), onclick: () => { pickLog(l.name, true); } }, l.name))),
      h('span.dv-grow'),
      h('label.dv-label', {}, 'Follow', h('input.dv-switch', { type: 'checkbox', role: 'switch', checked: L.follow,
        onchange: (e) => { L.follow = e.target.checked; if (L.follow) readLog(); } })),
      btn('Clear', async () => {
        const name = L.name;
        if (await ask('Clear ' + name + '?', 'Its contents are deleted.', 'Clear', true))
          act('Clearing…', () => daemon({ cmd: 'log_clear', name }), () => { pickLog(name, false); return 'Cleared ' + name; });
      })),
    h('div.dv-sub.dv-mono.dv-note', {}, (S.status.home || '~/.devid') + '/log/' + L.name),
    box,
  ];
}

function pickLog(name, render) {
  Object.assign(S.log, { name, offset: null, lines: [] });
  if (render) renderPage();
  if (name) readLog();
}

async function readLog() {
  const L = S.log;
  const name = L.name;
  let r;
  try { r = await daemon({ cmd: 'log_read', name, offset: L.offset }); } catch { return; }
  if (name !== L.name) return;
  if (L.offset !== null && r.offset < L.offset) L.lines = []; // the file was cleared or rotated
  L.offset = r.offset;
  if (!r.text) return;
  const fresh = r.text.split('\n');
  if (fresh[fresh.length - 1] === '') fresh.pop();
  L.lines = L.lines.concat(fresh).slice(-2000);
  const box = $('logbox');
  if (box) fillLog(box);
}

function fillLog(box) {
  const lines = S.log.lines;
  box.replaceChildren(...(lines.length ? lines.map((t) =>
    h('div.dv-ln' + (/error|crit|fatal|emerg|alert/i.test(t) ? '.error' : /warn/i.test(t) ? '.warning' : ''), {}, t)) :
    [h('div.dv-sub.dv-empty', {}, S.log.name + ' is empty.')]));
  if (S.log.follow || S.logPanel) box.scrollTop = box.scrollHeight;
}

/** What the daemon's automatic check found (D-028): a newer devid and the binary changes. */
function autoUpdates(st) {
  const u = st?.updates;
  if (!u?.checked_at) return null;
  const changes = (u.changes || []).map((c) => c.what + ' ' + c.from + '→' + c.to);
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
  const binaries = S.updates || (auto?.changes.length ? { text: 'Available: ' + auto.changes.join(', '), changes: auto.changes } : null);
  const devidText = S.devid?.text || (auto?.devid ? 'devid ' + auto.devid + ' is available (installed: ' + st.version + ')' : '');
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
    card(
      row(text('Devid ' + st.version, devidText || 'Updates the Devid app itself'),
        S.devid?.available || (!S.devid && auto?.devid) ? btn('Update Devid…', updateDevid, 'primary') : null,
        busyBtn('devid-check', 'Check for updates', () => act('Checking…', async () => {
          const line = await invoke('devid_check');
          S.devid = { text: line, available: / is available /.test(line) };
        }, null, 'devid-check'))),
      row(text('PHP, web server and extensions', binaries?.text || 'Newer builds from Devid\'s release'),
        binaries?.changes.length ? busyBtn('updates-apply', 'Update', updateBinaries, 'primary') : null,
        busyBtn('updates-check', 'Check for updates', () => act('Checking…', async () => {
          S.updates = updateSummary(await daemon({ cmd: 'update', check: true }), true);
        }, null, 'updates-check')))),
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

async function updateDevid() {
  if (await ask('Update Devid?', 'Terminal opens and runs devid self-update, which installs the new version and restarts the daemon.', 'Update'))
    act('Opening Terminal…', () => invoke('in_terminal', { action: 'self-update' }), (m) => m);
}

async function updateBinaries() {
  const list = (S.updates || autoUpdates(S.status)).changes.join(', ');
  if (await ask('Install the updates?', 'Downloads and installs ' + list + ' from Devid\'s release (sha256-checked).', 'Update'))
    act('Updating…', async () => { S.updates = updateSummary(await daemon({ cmd: 'update', check: false }), false); return S.updates.text; }, (t) => t, 'updates-apply');
}

/** The TUI's summary of an `update` reply (crates/cli/src/tui/mod.rs). */
function updateSummary(v, check) {
  const changes = (v.changes || []).map((c) => c.what + ' ' + c.from + '→' + c.to);
  const unknown = v.unknown || [];
  let text = !changes.length ? (unknown.length ? 'No updates found' : 'Everything is up to date (release ' + v.release + ')')
    : (check ? 'Available: ' : 'Updated: ') + changes.join(', ');
  if (unknown.length) text += ' · version unknown, skipped: ' + unknown.join(', ') + ' (try again)';
  return { text, changes: check ? changes : [] };
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
  const key = JSON.stringify([S.page, S.site, heading, PAGES[S.page].sub(), !!S.busy, cardBusy, autoUpdates(st)?.count,
    st && [st.sites, st.default_php, st.server, st.server_status, st.php.map((p) => p.status)]]);
  if (key === chromeKey) return;
  chromeKey = key;
  $('nav').replaceChildren(...Object.entries(PAGES).map(([key, p]) =>
    h('button.dv-nav' + (key === S.page ? '.is-active' : ''), { type: 'button', 'aria-current': key === S.page ? 'page' : null,
      onclick: () => go(key) }, icon(key), h('span.dv-grow', {}, p.title),
    key === 'sites' && st ? h('span.dv-chip', {}, String(st.sites)) : key === 'php' && st ? h('span.dv-chip', {}, st.default_php) :
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
    return;
  }
  const failed = st.php.filter((p) => p.status === 'failed').length + (st.server_status === 'failed' ? 1 : 0);
  const running = st.server_status === 'running';
  $('status-dot').className = 'dv-dot ' + (failed ? 'bad' : running ? 'ok' : 'warn');
  $('status-server').textContent = st.server;
  $('status-line').textContent = cardBusy || ((failed ? plural(failed, 'service') + ' failed' : capital(st.server_status)) + ' · PHP ' + st.default_php);
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
  if (!S.status) {
    page.replaceChildren(h('div.dv-banner', { role: 'status' }, h('span.dv-dot.bad'),
      text(h('span.dv-strong', {}, 'Devid is not running'), S.error || '')));
    return;
  }
  const data = S.data[S.page];
  if (!data) return;
  const scroll = page.parentElement.scrollTop;
  page.replaceChildren(...[versionBanner(), PAGES[S.page].render(data)].flat().filter(Boolean));
  page.parentElement.scrollTop = scroll;
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
    if (S.log.name && ((S.page === 'logs' && S.log.follow) || (S.page === 'sites' && S.logPanel))) readLog();
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
