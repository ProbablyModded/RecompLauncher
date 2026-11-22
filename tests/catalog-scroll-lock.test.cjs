// The Catalog's scroll moves only when the player moves it (ui/scroll-lock.js). Add and Install kept
// scrolling the Catalog on Windows after the causes were fixed one by one, so the page's position is
// now kept and any change the player didn't make is put back before it's painted. The player's own
// scrolling (wheel, touch, scrollbar, keys, controller) still works, and so do page changes and Back.
const { test } = require('node:test');
const assert = require('node:assert/strict');
const fs = require('node:fs');
const path = require('node:path');
const vm = require('node:vm');
const src = fs.readFileSync(path.join(__dirname, '../ui/scroll-lock.js'), 'utf8');
const app = fs.readFileSync(path.join(__dirname, '../ui/app.js'), 'utf8');
const html = fs.readFileSync(path.join(__dirname, '../ui/index.html'), 'utf8');
const rust = fs.readFileSync(path.join(__dirname, '../src-tauri/src/lib.rs'), 'utf8');
const push = fs.readFileSync(path.join(__dirname, '../src-tauri/src/push.rs'), 'utf8');

function world() {
  let clock = 1000, y = 0, observer = null;
  const on = {}, mainOn = {};
  const main = {
    dataset: { v: 'discover' }, scrollHeight: 10000, clientHeight: 800,
    get scrollTop() { return y; },
    set scrollTop(v) { y = Math.max(0, Math.min(this.scrollHeight - this.clientHeight, v)); },
    addEventListener: (k, f) => { mainOn[k] = f; },
    contains: (x) => x === main || (x && x.inMain),
  };
  const body = { classList: { has: new Set(), contains(c) { return this.has.has(c); } } };
  const ctx = vm.createContext({
    document: { getElementById: () => main, createElement: () => ({}), head: { appendChild() {} }, body },
    window: {}, performance: { now: () => clock },
    MutationObserver: class { constructor(f) { observer = f; } observe() {} },
    addEventListener: (k, f) => { on[k] = f; },
  });
  vm.runInContext(src, ctx);
  const w = {
    main, ctx, body,
    // Something moved the page (focus, anchoring, a redraw...) and the browser fires scroll.
    jump(to) { y = to; mainOn.scroll(); },
    // The player's own scroll: the event first, then the browser moves it.
    wheel(dy) { on.wheel({}); y += dy; mainOn.scroll(); },
    key(key, target) { on.keydown({ key, target: target || { closest: () => null } }); },
    redraw() { observer(); },
    later(ms) { clock += ms; },
    get y() { return y; },
    set y(v) { y = v; },
    on,
  };
  return w;
}

test('on the Catalog, a scroll the player did not make is put back', () => {
  const w = world();
  w.wheel(3000);
  w.later(2000);
  w.jump(4200);
  assert.equal(w.y, 3000);
  w.jump(0);
  assert.equal(w.y, 3000);
});

test('a redraw that moved the page is put back as soon as it is done', () => {
  const w = world();
  w.wheel(1500);
  w.later(2000);
  w.y = 2600; // moved during the redraw, before any scroll event
  w.redraw();
  assert.equal(w.y, 1500);
});

test('the wheel, the keys that scroll and the controller still move it', () => {
  const w = world();
  w.wheel(1000);
  assert.equal(w.y, 1000);
  w.later(2000);
  w.key('PageDown');
  w.jump(1700);
  assert.equal(w.y, 1700);
  w.later(2000);
  w.ctx.window.userScroll(900);
  w.jump(1900);
  assert.equal(w.y, 1900);
});

test('Enter or Space on a button (Add and Install by keyboard) does not unlock it', () => {
  const w = world();
  w.wheel(1000);
  w.later(2000);
  const button = { closest: () => ({}) };
  w.key('Enter', button);
  w.key(' ', button);
  w.jump(2500);
  assert.equal(w.y, 1000);
});

test('dragging the page scrollbar moves it, and it locks again after', () => {
  const w = world();
  w.on.pointerdown({ target: { closest: (s) => (s === '.vbar' ? {} : null) }, button: 0 });
  w.later(5000);
  w.jump(3000);
  assert.equal(w.y, 3000);
  w.on.pointerup({ pointerType: 'mouse' });
  w.later(2000);
  w.jump(100);
  assert.equal(w.y, 3000);
});

test('a touch drag and its glide move it', () => {
  const w = world();
  w.on.touchstart({});
  w.jump(800);
  w.on.touchend({ touches: [] });
  w.later(1000);
  w.jump(1100);
  assert.equal(w.y, 1100);
  w.later(2000);
  w.jump(200);
  assert.equal(w.y, 1100);
});

test('other pages and Big Picture are not locked, and a page change takes the new place', () => {
  const w = world();
  w.wheel(2000);
  w.later(2000);
  w.main.dataset.v = 'game';
  w.redraw();
  w.jump(0);
  assert.equal(w.y, 0);
  // Back to the Catalog where it was (back() writes the scroll after drawing).
  w.main.dataset.v = 'discover';
  w.y = 2000;
  w.redraw();
  w.jump(50);
  assert.equal(w.y, 2000);
  w.body.classList.has.add('tv-on');
  w.jump(50);
  assert.equal(w.y, 50);
});

test('a page that got shorter keeps the place it had to move to', () => {
  const w = world();
  w.wheel(5000);
  w.later(2000);
  w.main.scrollHeight = 2000;
  w.y = 1200;
  w.redraw();
  assert.equal(w.y, 1200);
  w.main.scrollHeight = 10000;
  w.jump(3000);
  assert.equal(w.y, 1200);
});

test('scroll anchoring is off on the Catalog, and the lock loads after the scrollbar', () => {
  assert.match(src, /#main\[data-v="discover"\]\{overflow-anchor:none\}/);
  assert.ok(html.indexOf('scroll-lock.js') > html.indexOf('scrollbar.js'));
  assert.match(app, /function padReveal\(el, behavior\) \{\n[^\n]*\n  if \(typeof userScroll === 'function'\) userScroll\(900\);/);
  assert.match(app, /function revealEnded[^]*?userScroll\(800\)/);
});

// ---- Quiver with the Catalog's refresh ----
test('a Catalog refresh syncs Quiver too when the sync is on', () => {
  const fn = rust.match(/fn quiver_enabled[^]*?\n}\n/)[0];
  assert.match(fn, /"auto"/);
  assert.match(fn, /quiver_hide/);
  const get = rust.match(/async fn get_catalog[^]*?\n}\n/)[0];
  // Refresh pressed: synced before the list is sent back, so it has Quiver's games.
  assert.match(get, /if force && !f\.offline && quiver_enabled\(&st\) \{\s*quiver_with_refresh\(&app, true\)\.await;/);
  // A list that was old, refreshed in the background.
  assert.match(get, /"catalog-fresh"[^]*?quiver_with_refresh\(&app, false\)\.await/);
  // Back online, and the site's push.
  assert.match(rust.match(/fn back_online[^]*?\n}\n/)[0], /quiver_with_refresh\(&app, false\)\.await/);
  assert.match(push, /quiver_with_refresh\(&app, false\)\.await/);
});
