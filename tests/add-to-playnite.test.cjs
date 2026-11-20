const { test } = require('node:test');
const assert = require('node:assert/strict');
const fs = require('node:fs');
const path = require('node:path');
const vm = require('node:vm');

const app = fs.readFileSync(path.join(__dirname, '../ui/app.js'), 'utf8');
const tv = fs.readFileSync(path.join(__dirname, '../ui/tv.js'), 'utf8');
const games = fs.readFileSync(path.join(__dirname, '../ui/games.js'), 'utf8');
const fn = (src, name, indent = '') => src.match(new RegExp(`^${indent}(?:async )?function ${name}\\([^]*?^${indent}}`, 'm'))[0];

// addToPlaynite with the core's answers in order, and what the player was told and asked.
function run(answers, pick) {
  const calls = [], toasts = [], asked = [];
  const c = vm.createContext({
    S: { library: { zelda: { name: 'Zelda' } } },
    t: (s) => s, esc: String,
    tf: (s, values) => s.replace(/\{(\w+)\}/g, (_, k) => values[k]),
    toast: (m) => toasts.push(m),
    fail: (e) => { throw e; },
    invoke: async (cmd, args) => { calls.push([cmd, args]); return answers.shift(); },
    ask: async (title) => { asked.push(title); return pick; },
    window: {},
  });
  vm.runInContext(fn(app, 'addToPlaynite'), c);
  return { c, calls, toasts, asked };
}

test('Playnite took it: one call, and it says so', async () => {
  const r = run(['added']);
  await r.c.addToPlaynite('zelda', false);
  assert.equal(JSON.stringify(r.calls), JSON.stringify([['add_to_playnite', { id: 'zelda', restart: false }]]));
  assert.deepEqual(r.toasts, ['Zelda is in your Playnite library.']);
});

test('Playnite closed: the game waits for it', async () => {
  const r = run(['queued']);
  await r.c.addToPlaynite('zelda', false);
  assert.deepEqual(r.toasts, ['Zelda goes in the next time Playnite starts.']);
});

test('Playnite open without the add-on: asks before restarting it', async () => {
  const r = run(['playnite_restart', 'restarted'], 'yes');
  await r.c.addToPlaynite('zelda', false);
  assert.deepEqual(r.asked, ['Restart Playnite?']);
  assert.deepEqual(r.calls.map((x) => x[1].restart), [false, true]);
  assert.equal(r.toasts.at(-1), 'Playnite is opening again, with Zelda in your library.');
});

test('Not now leaves Playnite open, and the game goes in later', async () => {
  const r = run(['playnite_restart'], null);
  await r.c.addToPlaynite('zelda', false);
  assert.equal(r.calls.length, 1);
  assert.deepEqual(r.toasts, ['Zelda goes in the next time Playnite starts.']);
});

test('Add to Playnite sits next to Add to Steam, only when Playnite is on the PC', () => {
  assert.match(app, /S\.steam \? \[\{ act: 'steam'[^\n]*\n\s*\.\.\.\(S\.playnite \? \[\{ act: 'playnite', id, label: t\('Add to Playnite'\)/);
  assert.match(app, /case 'playnite': return addToPlaynite\(id, false\);/);
  assert.match(tv, /S\.playnite && window\.addToPlaynite \? \[\{ label: t\('Add to Playnite'\)/);
  // One game of several is added under its own id, like Add to Steam.
  assert.match(games, /'add_to_steam', 'add_to_playnite'/);
});
