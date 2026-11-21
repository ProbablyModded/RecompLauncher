const { test } = require('node:test');
const assert = require('node:assert/strict');
const fs = require('node:fs');
const path = require('node:path');
const vm = require('node:vm');

const app = fs.readFileSync(path.join(__dirname, '../ui/app.js'), 'utf8');
const qv = fs.readFileSync(path.join(__dirname, '../ui/quiver.js'), 'utf8');
const html = fs.readFileSync(path.join(__dirname, '../ui/index.html'), 'utf8');

// quiver.js in a sandbox: what it puts on window, with plain stand-ins around it.
function load(ui = {}) {
  const c = vm.createContext({ S: { settings: { ui }, view: 'catalog' }, t: (s) => s, tf: (s) => s, esc: String, fmtNum: String, ago: String, invoke: async () => ({}), render: () => {}, toast: () => {}, fail: (e) => { throw e; }, loadCatalog: async () => {}, saveUi: async () => {} });
  c.window = c;
  vm.runInContext(qv, c);
  return c;
}

test('the Quiver section is loaded after app.js and reached through generic hooks', () => {
  assert.ok(html.indexOf('quiver.js') > html.indexOf('app.js'));
  assert.match(app, /window\.importGroup \? window\.importGroup\(group, row, sw\)/);
  assert.match(app, /globalThis\.importBadge \? globalThis\.importBadge\(a\)/);
  assert.match(app, /window\.importCard \? window\.importCard\(a\)/);
  assert.match(app, /\/\^qv-\/\.test\(a\) && window\.qvAct/);
});

test('an imported game shows in the Catalog until RecompLauncher has it, or the player hides them', () => {
  const c = load();
  assert.equal(c.importShown({ id: 'native' }), true);
  assert.equal(c.importShown({ id: 'qv-a', quiver: {} }), true);
  assert.equal(c.importShown({ id: 'qv-a', quiver: {}, superseded_by: 'a' }), false);
  assert.equal(load({ quiver_hide: true }).importShown({ id: 'qv-a', quiver: {} }), false);
  assert.equal(load({ quiver_hide: true }).importShown({ id: 'native' }), true);
});

test('only imported games get the badge and the card', () => {
  const c = load();
  assert.equal(c.importBadge({ id: 'x' }), '');
  assert.match(c.importBadge({ id: 'qv-a', quiver: {} }), /q-badge/);
  assert.equal(c.importCard({ id: 'x' }), '');
  assert.match(c.importCard({ id: 'qv-a', quiver: { url: 'https://quiverlauncher.com/apps/a' } }), /See It on Quiver/);
  assert.doesNotMatch(c.importCard({ id: 'qv-a', quiver: { url: 'https://evil.example/apps/a' } }), /data-url/, 'only links to Quiver\'s own site');
});

test('Catalog visibility goes through the hook', () => {
  assert.match(app, /const visible = \(\) => \{[^\n]*importShown\(a\)/);
});

test('the details form sends the extra fields and the release file for this system', () => {
  const start = app.indexOf('function gameInfoModal(');
  const body = app.slice(start, app.indexOf('\n}', start));
  for (const id of ['gi-author', 'gi-genre', 'gi-first', 'gi-website', 'gi-tags', 'gi-blurb', 'gi-notes', 'gi-pattern']) assert.ok(body.includes(`id="${id}"`), id);
  assert.match(body, /assets: Object\.assign\(\{\}, info\.assets \|\| \{\}, \{ \[S\.os\]: pattern \}\)/);
  assert.match(body, /must|https:\\\/\\\//);
});

test('every catalog game can have its details edited, not only games added by link', () => {
  const start = app.indexOf('function editInfo(');
  const body = app.slice(start, app.indexOf('\n}', start));
  assert.match(body, /if \(!a\.custom\) return gameDetailsModal\(a\);/);
  assert.doesNotMatch(app, /x && x\.custom \? \[\{ act: 'edit-info'/);
  const modal = app.slice(app.indexOf('function gameDetailsModal('), app.indexOf('\nfunction editInfo('));
  assert.match(modal, /invoke\('set_game_info', \{ id, info, reset: x\.dataset\.x === 'reset' \}\)/);
  assert.match(modal, /Reset to Catalog/);
});

test('the Catalog has a Source filter once Quiver games are showing', () => {
  const games = { apps: [{ id: 'n' }, { id: 'qv-a', quiver: {} }] };
  const c = load();
  c.S.cat = games;
  assert.deepEqual(JSON.parse(JSON.stringify(c.importSources())), [['quiver', 'Quiver']]);
  assert.equal(c.importFrom({ id: 'qv-a', quiver: {} }), 'quiver');
  assert.equal(c.importFrom({ id: 'n' }), '');
  const off = load({ quiver_hide: true }); off.S.cat = games;
  assert.equal(off.importSources().length, 0);
  const none = load(); none.S.cat = { apps: [{ id: 'n' }, { id: 'qv-b', quiver: {}, superseded_by: 'b' }] };
  assert.equal(none.importSources().length, 0);
  assert.match(app, /srcOk\(a, f\.src\)/);
  assert.match(app, /data-act="gh-add"[^\n]*Add a Recomp[^\n]*data-go="discover"/);
});
