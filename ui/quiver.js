'use strict';
// Quiver Catalog Sync (fork; Settings → Games and folders): adds the games Quiver Launcher's
// catalog lists that RecompLauncher's own catalog doesn't, matched by their project's repository,
// never by title. quiver.rs and merge_quiver in lib.rs do the work; this is the section.

const QV = { data: null, loading: false, busy: false };

async function qvLoad() {
  if (QV.loading) return;
  QV.loading = true;
  try { QV.data = await invoke('quiver_status'); } catch (e) { QV.data = { count: 0, synced: 0, stats: null }; }
  QV.loading = false;
  if (S.view === 'settings') render(true);
}

function qvSummary(d) {
  if (!d.synced) return t('Not synced yet. Sync to add the games Quiver Launcher has that RecompLauncher doesn\'t.');
  const st = d.stats || {};
  const parts = [tf('{n} games added', { n: fmtNum(d.count || 0) })];
  if (st.quiver) parts.push(tf('{n} of Quiver\'s {total} were already in RecompLauncher', { n: fmtNum(st.matched || 0), total: fmtNum(st.quiver) }));
  if (st.tools) parts.push(tf('{n} tools and emulators left out', { n: fmtNum(st.tools) }));
  if (st.not_desktop) parts.push(tf('{n} only for phones', { n: fmtNum(st.not_desktop) }));
  return `${tf('Last synced {when}', { when: ago(d.synced) })}: ${parts.join(' · ')}`;
}

function quiverGroup(group, row, sw) {
  if (!QV.data) { qvLoad(); return group(t('Quiver Catalog'), `<p class="set-note">${esc(t('Loading…'))}</p>`); }
  const d = QV.data, ui = S.settings.ui || {};
  const note = `<p class="set-note">${esc(t('Adds the games in Quiver Launcher\'s catalog that RecompLauncher\'s own catalog doesn\'t have. Games are matched by their project\'s repository, never by their name. They show in the Catalog with a Quiver tag, install and update from their project\'s releases, and work with Data Hoarder, Folder Structure and Add to Playnite. When RecompLauncher adds one of them later, your copy moves over to RecompLauncher\'s own entry and its setup by itself.'))}</p>`;
  const syncBtn = `<button type="button" class="btn sm main" data-act="qv-sync"${QV.busy ? ' disabled' : ''}>${esc(QV.busy ? t('Syncing…') : t('Sync Now'))}</button>`;
  const clear = d.count ? `<button type="button" class="btn sm danger" data-act="qv-clear">${esc(t('Remove Them'))}</button>` : '';
  return group(t('Quiver Catalog'), note
    + row(t('Quiver Games'), qvSummary(d), `<span class="acts">${clear}${syncBtn}</span>`)
    + row(t('Keep in Sync'), t('Syncs again once a day, in the background.'), sw('qv_auto', !!d.auto, t('Keep in Sync'), 'qv-auto'))
    + row(t('Include Tools and Emulators'), t('Off: only games and ports. On: Quiver\'s tools and emulators too. Takes effect on the next sync.'), sw('qv_tools', !!d.include_tools, t('Include Tools and Emulators'), 'qv-tools'))
    + row(t('Show Quiver Games in the Catalog'), t('Off: they stay out of the Catalog. Ones in your Library stay there either way.'), sw('qv_show', !ui.quiver_hide, t('Show Quiver Games in the Catalog'), 'qv-show')));
}
window.importGroup = quiverGroup;

// Shown in the Catalog unless RecompLauncher's own catalog has it now (superseded_by), or the
// player turned Quiver games off.
window.importShown = (a) => !a.quiver || (!a.superseded_by && !(S.settings.ui || {}).quiver_hide);
// The Catalog's Source filter: Quiver once some of its games are showing.
window.importSources = () => ((S.settings.ui || {}).quiver_hide || !((S.cat && S.cat.apps) || []).some((a) => a.quiver && !a.superseded_by) ? [] : [['quiver', 'Quiver']]);
window.importFrom = (a) => (a && a.quiver ? 'quiver' : '');
window.importBadge = (a) => (a.quiver ? `<span class="q-badge" title="${esc(t('From Quiver Launcher\'s Catalog'))}">Quiver</span> ` : '');
window.importCard = (a) => {
  if (!a.quiver) return '';
  const q = a.quiver, url = /^https:\/\/quiverlauncher\.com\//.test(q.url || '') ? q.url : '';
  const gone = a.superseded_by ? `<p style="margin:8px 0 0">${esc(t('RecompLauncher\'s own catalog has it now. Your copy moves over to that entry as soon as nothing is running for it.'))}</p>` : '';
  return `<div class="card src-card"><div class="card-h" style="margin-bottom:8px"><h3>${esc(t('From Quiver Launcher\'s Catalog'))}</h3>${url ? `<button type="button" class="btn sm" data-act="url" data-url="${esc(url)}">${esc(t('See It on Quiver'))}</button>` : ''}</div>
    <p class="muted" style="margin:0">${esc(t('RecompLauncher\'s own catalog doesn\'t list this game yet, so it came from Quiver Launcher\'s catalog. It installs and updates from its project\'s releases. If RecompLauncher adds it later, your copy moves over to RecompLauncher\'s own entry by itself.'))}</p>${gone}</div>`;
};

window.qvAct = async (a) => {
  try { await qvRun(a); } catch (e) { QV.busy = false; fail(e); render(true); }
};

async function qvRun(a) {
  if (a === 'qv-sync') {
    if (QV.busy) return;
    QV.busy = true; render(true);
    QV.data = await invoke('quiver_sync', { includeTools: null, auto: null });
    QV.busy = false;
    await loadCatalog(false);
    toast(tf('Synced. {n} games from Quiver\'s catalog are in the Catalog.', { n: fmtNum(QV.data.count || 0) }));
    return render(true);
  }
  if (a === 'qv-auto') { QV.data = await invoke('quiver_options', { auto: !QV.data.auto, includeTools: null }); return render(true); }
  if (a === 'qv-tools') { QV.data = await invoke('quiver_options', { auto: null, includeTools: !QV.data.include_tools }); return render(true); }
  if (a === 'qv-show') { await saveUi({ quiver_hide: !(S.settings.ui || {}).quiver_hide }, true); return render(true); }
  if (a === 'qv-clear') {
    QV.data = await invoke('quiver_clear');
    await loadCatalog(false);
    toast(t('Removed. Ones in your Library stay.'));
    return render(true);
  }
}
