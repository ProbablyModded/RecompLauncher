'use strict';
// Fork: the Catalog's scroll moves only when the player moves it.
//
// Add and Install (and Add to Library with Data Hoarder on) kept scrolling the Catalog on Windows,
// after three rounds of fixing the causes one at a time (a closed menu, a focused button taken out,
// redraws writing the scroll back). WebView2 can move a scroll box for more reasons than the page
// can see: focus landing somewhere, scroll anchoring, a redraw, an element going away. So this
// doesn't chase causes. On the Catalog, the page's scroll position is kept, and any change the
// player didn't make is put back before it's painted.
//
// What counts as the player's own: the mouse wheel or touchpad, a touch drag (and its glide), the
// page's scrollbar (dragging it or clicking its track), a middle-click autoscroll, the keys that
// scroll (arrows, Page Up/Down, Home, End, Space outside a button, Tab), and the controller moving
// its highlight (app.js calls window.userScroll for that). Changing pages, Back, and opening Ended
// Projects (the player asked for those) move it as before.
//
// When the page gets shorter than where it was (a search with fewer results), the browser has to
// move it: that position is kept from then on.
(() => {
  const main = document.getElementById('main');
  if (!main) return;
  // Scroll anchoring (Chromium, so WebView2) moves the page when something above the view changes
  // size. Off on the Catalog: what's on screen stays where it is.
  const css = document.createElement('style');
  css.textContent = '#main[data-v="discover"]{overflow-anchor:none}';
  document.head.appendChild(css);

  const L = { y: main.scrollTop, v: main.dataset.v || '', until: 0, held: false, auto: false };
  const now = () => performance.now();
  const on = () => main.dataset.v === 'discover' && !document.body.classList.contains('tv-on');
  const free = () => L.held || L.auto || now() < L.until;
  const allow = (ms) => { L.until = Math.max(L.until, now() + (ms || 800)); };
  const keep = () => { L.y = main.scrollTop; L.v = main.dataset.v || ''; };
  // The player's moves (and app.js's controller highlight) let the page move for a moment.
  window.userScroll = (ms) => { allow(ms); keep(); };

  function check() {
    if (main.dataset.v !== L.v || !on() || free()) return keep();
    if (Math.abs(main.scrollTop - L.y) < 1) return;
    main.scrollTop = L.y;
    // Couldn't go back (the page is shorter now): where it had to go is the new place.
    if (Math.abs(main.scrollTop - L.y) >= 1) L.y = main.scrollTop;
  }
  window.scrollLockCheck = check;
  // A scroll event comes before the frame is painted, so a jump is put back before it's seen.
  main.addEventListener('scroll', check, { passive: true });
  // A redraw (or a page change) is looked at as soon as it's done, before any scroll event.
  new MutationObserver(check).observe(main, { childList: true, subtree: true, attributes: true, attributeFilter: ['data-v'] });

  const opts = { passive: true, capture: true };
  // Wheel and touchpad: each event lets the glide that follows it finish.
  addEventListener('wheel', () => allow(700), opts);
  // Touch: held while a finger is down, then long enough for the flick's glide.
  addEventListener('touchstart', () => { L.held = true; }, opts);
  const touchEnd = (e) => { if (!e.touches || !e.touches.length) { L.held = false; allow(1600); } };
  addEventListener('touchend', touchEnd, opts);
  addEventListener('touchcancel', touchEnd, opts);
  addEventListener('pointerdown', (e) => {
    L.auto = false;
    // The page's own scrollbar (scrollbar.js): its thumb dragged, or its track clicked (a smooth page).
    if (e.target && e.target.closest && e.target.closest('.vbar')) { L.held = true; allow(900); return; }
    // Middle-click autoscroll: until the next click or key.
    if (e.button === 1 && main.contains(e.target)) L.auto = true;
  }, opts);
  const up = () => { if (L.held) { L.held = false; allow(900); } };
  addEventListener('pointerup', (e) => { if (e.pointerType !== 'touch') up(); }, opts);
  addEventListener('pointercancel', (e) => { if (e.pointerType !== 'touch') up(); }, opts);
  // Keys that scroll the page, or move focus (and the page with it) on purpose.
  const KEYS = new Set(['ArrowUp', 'ArrowDown', 'ArrowLeft', 'ArrowRight', 'PageUp', 'PageDown', 'Home', 'End', 'Tab']);
  addEventListener('keydown', (e) => {
    L.auto = false;
    const el = e.target, onButton = el && el.closest && el.closest('button, a, input, textarea, select, [role="button"], [contenteditable="true"]');
    if (KEYS.has(e.key) || (e.key === ' ' && !onButton)) allow(900);
  }, opts);
})();
