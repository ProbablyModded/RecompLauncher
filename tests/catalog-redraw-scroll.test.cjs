// The Catalog must stay where the player left it while it's redrawn in the background (a download
// ticking, a release list coming in, a game added). Three things moved it:
// - resting the pointer on a tile asked for its release list, and any answer redrew the whole page;
// - the focused control was taken out or moved by the redraw, and WebView2 (Windows) scrolls the page
//   when the focused element leaves it;
// - the scroll was written back on every redraw, which stops a wheel or touchpad glide in progress and,
//   after a long redraw, puts the page back where it was when the redraw began.
const { test } = require('node:test');
const assert = require('node:assert/strict');
const fs = require('node:fs');
const path = require('node:path');
const vm = require('node:vm');
const app = fs.readFileSync(path.join(__dirname, '../ui/app.js'), 'utf8');
const fn = (name) => app.match(new RegExp(`^(?:async )?function ${name}\\([^]*?^}`, 'm'))[0];
const line = (name) => app.match(new RegExp(`^const ${name} = [^\\n]*`, 'm'))[0];

// ---- release lists arriving ----
function relsContext(response) {
  const ctx = vm.createContext({ S: { rels: {}, view: 'discover' }, t: (s) => s, tn: (n, one, many) => (n === 1 ? one : many),
    invoke: async () => response, render: () => { ctx.redraws++; }, redraws: 0, window: {}, document: { body: { classList: { contains: () => false } } } });
  for (const name of ['projectState', 'loadRels']) vm.runInContext(fn(name), ctx);
  return ctx;
}

test('a release list for a project that is not archived does not redraw the Catalog', async () => {
  for (const archived of [false, undefined]) {
    const ctx = relsContext({ data: [], archived });
    await vm.runInContext("loadRels('owner/game')", ctx);
    assert.equal(ctx.redraws, 0);
  }
});

test('a project found to be archived still redraws once, and again when that changes back', async () => {
  const response = { data: [], archived: true };
  const ctx = relsContext(response);
  await vm.runInContext("loadRels('owner/game')", ctx);
  assert.equal(ctx.redraws, 1);
  await vm.runInContext("loadRels('owner/game', true)", ctx);
  assert.equal(ctx.redraws, 1);
  response.archived = false;
  await vm.runInContext("loadRels('owner/game', true)", ctx);
  assert.equal(ctx.redraws, 2);
});

// ---- focus across a redraw ----
// A tiny page: #main holding tiles, each with its main button and a quick button.
function page() {
  const doc = { activeElement: null };
  const log = [];
  const el = (tag, data, label) => {
    const e = {
      tagName: tag, dataset: Object.assign({}, data), parent: null, kids: [], isContentEditable: false,
      getAttribute: (k) => (k === 'aria-label' ? label || null : null),
      contains(x) { for (let n = x; n; n = n.parent) if (n === e) return true; return false; },
      closest(sel) { for (let n = e; n; n = n.parent) if (sel === '[data-id]' ? n.dataset.id : false) return n; return null; },
      focus(opts) { log.push(`${e.dataset.act || tag}:${e.dataset.id || ''}${opts && opts.preventScroll ? ':noscroll' : ':SCROLL'}`); doc.activeElement = e; },
      get isConnected() { for (let n = e; n; n = n.parent) if (n === main) return true; return false; },
      add(...k) { for (const c of k) { c.parent = e; e.kids.push(c); } return e; },
      remove() { if (!e.parent) return; e.parent.kids = e.parent.kids.filter((c) => c !== e); e.parent = null; if (e.contains(doc.activeElement)) doc.activeElement = body; },
    };
    return e;
  };
  const body = el('BODY', {});
  const main = el('MAIN', {});
  const tile = (id, quick) => el('DIV', { id }).add(el('BUTTON', { act: 'open', id }, 'Game ' + id), el('BUTTON', { act: quick, id }, quick + ' ' + id));
  main.add(tile('a', 'addlib'), tile('b', 'addlib'));
  const all = (root) => { const out = []; const walk = (n) => { for (const c of n.kids) { out.push(c); walk(c); } }; walk(root); return out; };
  const $$ = (sel, root) => all(root).filter((n) => (sel.startsWith('.tile[') ? n.tagName === 'DIV' && n.dataset.id : n.tagName === 'BUTTON'));
  const $ = (sel, root) => $$(sel, root)[0] || null;
  const ctx = vm.createContext({ document: doc, $, $$ });
  vm.runInContext(`${line('focusSig')}\n${fn('holdFocus')}`, ctx);
  return { ctx, doc, main, body, log, tile, el };
}

test('a focused + that a redraw takes away hands focus to its tile, never to the page body', () => {
  const p = page();
  const plus = p.main.kids[1].kids[1];
  plus.focus(); p.log.length = 0;
  const back = p.ctx.holdFocus(p.main);
  assert.equal(p.doc.activeElement, p.main, 'focus waits on the page while it changes');
  plus.remove(); // a download started: the tile has no quick button now
  assert.notEqual(p.doc.activeElement, p.body);
  back();
  assert.deepEqual(p.log, ['MAIN::noscroll', 'open:b:noscroll']);
});

test('a focused control drawn anew in its place gets focus back', () => {
  const p = page();
  const plus = p.main.kids[0].kids[1];
  plus.focus(); p.log.length = 0;
  const back = p.ctx.holdFocus(p.main);
  plus.remove();
  const again = p.el('BUTTON', { act: 'addlib', id: 'a' }, 'addlib a');
  p.main.kids[0].add(again);
  back();
  assert.equal(p.doc.activeElement, again);
  assert.deepEqual(p.log.slice(-1), ['addlib:a:noscroll']);
});

test('a focused tile that only moved keeps its focus', () => {
  const p = page();
  const open = p.main.kids[1].kids[0];
  open.focus(); p.log.length = 0;
  const back = p.ctx.holdFocus(p.main);
  const t = p.main.kids[1];
  t.remove(); p.main.add(t); // moved up the list
  back();
  assert.equal(p.doc.activeElement, open);
});

test('a field being typed in, focus outside the page, or a dialog that took focus are left alone', () => {
  const p = page();
  const field = p.el('INPUT', {});
  p.main.add(field);
  field.focus(); p.log.length = 0;
  p.ctx.holdFocus(p.main)();
  assert.deepEqual(p.log, []);
  const outside = p.el('BUTTON', { act: 'side' });
  outside.focus(); p.log.length = 0;
  p.ctx.holdFocus(p.main)();
  assert.deepEqual(p.log, []);
  const open = p.main.kids[0].kids[0];
  open.focus();
  const back = p.ctx.holdFocus(p.main);
  const dialog = p.el('BUTTON', { act: 'ok' });
  dialog.focus(); p.log.length = 0;
  back();
  assert.equal(p.doc.activeElement, dialog);
});

// ---- where the redraws use it ----
test('a redraw in place holds focus, and a list morph does too', () => {
  assert.match(fn('render'), /else \{ focusBack = holdFocus\(main\); morph\(main, html\); \}/);
  assert.match(fn('render'), /if \(focusBack\) focusBack\(\);/);
  assert.match(fn('morphList'), /focusBack = holdFocus\(main\);\s*try \{ morphListIn\(box, html, opts\); \} finally \{ focusBack\(\); \}/);
});

test('a redraw writes the scroll only when it really moved', () => {
  const r = fn('render');
  assert.match(r, /if \(Math\.abs\(main\.scrollTop - want\) >= 1\) main\.scrollTop = want;/);
  // No unconditional write back of the old position, and no += of a zero offset.
  assert.doesNotMatch(r, /if \(keepScroll\) main\.scrollTop = top;/);
  assert.doesNotMatch(r, /main\.scrollTop \+= /);
});
