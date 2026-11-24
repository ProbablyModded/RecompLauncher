// Closing a game's menu (Add to Library, Add and Install, any row) must not move the Catalog.
// WebView2 on Windows scrolls the page when the focused element is removed, and the focused
// element is the menu row that was just pressed, so focus has to leave the menu before it goes.
const { test } = require('node:test');
const assert = require('node:assert/strict');
const fs = require('node:fs');
const path = require('node:path');
const app = fs.readFileSync(path.join(__dirname, '../ui/app.js'), 'utf8');
const fn = (src, name) => src.match(new RegExp(`^(?:async )?function ${name}\\([^]*?^}`, 'm'))[0];

// A tiny page: #main scrolled to 2400, one open menu holding the focused row, and the control
// that opened it. Removing the menu while its row has focus jumps #main to the top, as WebView2 does.
function page() {
  const log = [];
  const main = { scrollTop: 2400 };
  const doc = { activeElement: null };
  const row = { blur: () => { log.push('blur'); doc.activeElement = null; } };
  const menu = {
    contains: (el) => el === row,
    remove: () => { log.push('remove'); if (doc.activeElement === row) main.scrollTop = 0; },
  };
  const opener = {
    isConnected: true,
    closest: () => null,
    focus: (opts) => { log.push('focus' + (opts && opts.preventScroll ? ':noscroll' : '')); doc.activeElement = opener; },
  };
  doc.activeElement = row;
  return { log, main, doc, menu, opener };
}
function closeMenuWith(p, back) {
  const src = `let MENU_BACK = back; ${fn(app, 'closeMenu')}; closeMenu();`;
  const $ = (sel) => (sel === '#main' ? p.main : null);
  const $$ = (sel) => (sel === '.menu' ? [p.menu] : []);
  new Function('$', '$$', 'document', 'back', src)($, $$, p.doc, back);
}

test('closing a menu puts focus back on its opener, without scrolling, before the menu goes', () => {
  const p = page();
  closeMenuWith(p, p.opener);
  assert.deepEqual(p.log, ['focus:noscroll', 'remove']);
});

test('closing a menu keeps the Catalog where it was', () => {
  const p = page();
  closeMenuWith(p, p.opener);
  assert.equal(p.main.scrollTop, 2400);
});

test('with no opener to go back to, focus leaves the menu before it goes', () => {
  const p = page();
  closeMenuWith(p, null);
  assert.deepEqual(p.log, ['blur', 'remove']);
  assert.equal(p.main.scrollTop, 2400);
});

test('an opener that was redrawn away is not focused', () => {
  const p = page();
  p.opener.isConnected = false;
  closeMenuWith(p, p.opener);
  assert.deepEqual(p.log, ['blur', 'remove']);
});

test('a right-click on a game remembers what was clicked, so focus goes back there', () => {
  assert.match(app, /loadStartWith\(id\)\.then\(\(\) => menu\(at, gameMenu\(id\), back\)\);/);
});

test('a menu opened from a button goes back to that button', () => {
  assert.match(fn(app, 'menu'), /MENU_BACK = back \|\| opener;/);
});
