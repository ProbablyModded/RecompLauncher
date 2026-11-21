# Quiver Catalog Sync, custom games and game details (fork)

Two ways to get games RecompLauncher's own catalog doesn't list, and a way to edit any game's
details, all in this fork only.

## 1. Custom games: Add a Recomp, with every detail

**Add a Recomp** (the button at the top of the Library) takes a project link: GitHub, GitLab, SourceForge,
or a Forgejo/Gitea server. Upstream already reads the project, suggests its game, console and art,
and adds it to the Library; Install downloads from the project's releases like any catalog game.

The button is also on an empty Library and under a Catalog search that finds nothing. A link to a
project the Catalog already has (RecompLauncher's own entry, or one imported from Quiver, compared by
repository) isn't added a second time: that entry goes into the Library (Not Installed, to install
whenever you like) and its page opens.

The fork's details form adds, under **More Details**:

| Field | Saved as | Filled in from the project |
|---|---|---|
| Developer | `author` | the repository owner |
| Genre | `genre` (the catalog's own genres) | no |
| First Released | `first` (YYYY-MM-DD) | no |
| Website | `website` (https only) | the repository's homepage |
| Tags | `tags` (a list, up to 20) | the repository's topics |
| Description | `blurb` | the repository's description |
| Your Notes | `notes` (Markdown) | no |
| Release File for this PC | `assets.<os>` (the catalog's pattern, `*` wildcards) | no: empty means the launcher picks it |

When the game is added the launcher also saves, once, what the project said about itself that day
(`snapshot`: its link, description, topics, license, homepage, stars, forks, default branch,
created and last-pushed dates, archived flag, how many releases, and the newest release's tag,
date, link and file names), plus `added`, `license` and `tag`. The snapshot is never refreshed, so it
still shows what the project was if it later changes or disappears. The game page shows a **Your
Game** card with the notes and the snapshot. **Edit Game Info…** changes everything but the snapshot.

Custom games are entries in `my-recomps.json`, so Data Hoarder Mode, Folder Structure and Add to
Playnite treat them like catalog games. When RecompLauncher's catalog later lists the same
repository, upstream's `merge_mine` moves the copy over to the catalog entry.

## Editing any game's details

**Edit Game Info…** (a game's "…" menu) works on every game now, not only games added by link. For
a game from RecompLauncher's catalog, or one imported from Quiver's, it edits Game, Console,
Developer, Genre, First Released, Website, Tags, Description and Your Notes. Name and art stay with
**Edit Name and Artwork…** (Customize), as upstream has it.

The changes are kept in `game-info.json` beside `library.json` (`{ id: { field: value } }`), never
in the catalog file, and are laid over the catalog last, after anything filled in from Quiver. So
what you write always wins, a catalog update never undoes it, and Folder Structure, Data Hoarder,
Add to Playnite and the Catalog's filters all use it. Each changed field is marked **Changed** in
the form, with the catalog's own value under it; leaving a field empty uses the catalog's value
again, and **Reset to Catalog** takes every change off. The repository, versions and release files
can't be changed this way (they decide what gets downloaded). When a Quiver game moves over to
RecompLauncher's own entry, your changes move with it. Notes (and a website you set) show on the
game's page in a **Your Notes** card.

`src-tauri/src/game_info.rs` (with unit tests), `set_game_info` in `lib.rs`, `gameDetailsModal` in
`ui/app.js`.

## 2. Quiver Catalog Sync

Settings → Games and folders → **Quiver Catalog** → **Sync Now**.

### Where the catalog comes from

Quiver Launcher reads `https://api.quiverlauncher.com/api/v1` (its source: `packages/api` in
tgeorgiadis/quiver-launcher). The sync reads two public lists, 100 at a time:

- `/release-status`: every app's `provider` (github, gitlab, gitea, manual), `host` and
  `repository`. This is what matching uses.
- `/apps`: every app's name, project name, game, consoles, description, art (`libraryArt`:
  capsule, header, hero, logo), tags, supported systems, project type, newest verified release and
  install hints (`launcher.releaseAssetFilter`, `launcher.preferredExecutables`).

When the main address can't be reached it asks the same API on its host's own address, as Quiver
Launcher does. Nothing about the player is sent. In November 2026 the catalog had 2,058 apps, 1,442
of them already in RecompLauncher's catalog by repository.

### Matching: by repository, never by title

Each app's repository is put in the launcher's own form, then compared in lowercase with any
`.git` and trailing slash taken off:

| Quiver | Launcher |
|---|---|
| `github`, `owner/name` (github.com) | `owner/name` |
| `gitlab`, `group/project` (gitlab.com) | `gitlab:group/project` |
| `gitea`, host `codeberg.org`, `owner/name` | `forgejo:codeberg.org/owner/name` |
| `manual`, a sourceforge.net project link | `sourceforge:name` |
| anything else (self-hosted GitLab, GitHub Enterprise, release feeds, other web pages) | not imported |

An app is **not** imported when its repository is already in RecompLauncher's catalog, or is one
of the player's own games added by link. Two apps with the same title but different repositories
are different games; two Quiver apps from one repository become one entry (they'd install the
same files).

Also left out: apps only for phones (no Windows, Linux or Mac build), and tools and emulators
unless **Include Tools and Emulators** is on.

### The Source filter

Once Quiver games are showing, the Catalog's filters get **Source**: Any, RecompLauncher (its own
catalog only) or Quiver (only games imported from Quiver). It's in the pinned filter bar's panel too,
and counts as an active filter there. `app.js` reads it through two generic hooks that `quiver.js`
sets: `importSources()` (the choices, empty while Quiver games are hidden) and `importFrom(a)`.

### What an imported game looks like

Entries live in `quiver-catalog.json` beside `library.json`, with ids `qv-<quiver slug>` (kept if
Quiver renames the app), and are joined to the catalog in `catalog::with_mine`, so Install, Data
Hoarder, Folder Structure and Add to Playnite see them like any game. They show in the Catalog with
a **Quiver** tag and a **From Quiver Launcher's Catalog** card on their page.

| Launcher field | From Quiver |
|---|---|
| `name` | project name (else the app name) |
| `game` | first game's title (else the app name) |
| `origin` | `basedOn.console`, else the first console, in RecompLauncher's ids (below) |
| `author` | developer name, else the repository owner |
| `blurb`, `description` | description |
| `cover`, `hero`, `logo`, `icon` | capsule, hero (else header), logo, artwork (https only) |
| `systems` | supported systems that are Windows, Linux or Mac |
| `tags` | tags (up to 12), in RecompLauncher's spelling where it has the tag |
| `tag`, `version`, `released`, `added` | newest verified release, dates as YYYY-MM-DD |
| `assets.<os>` | `*<releaseAssetFilter>*` (Quiver's filter is a piece of the file name) |
| `ai` | `aiLevel` when it isn't "none", so Hide AI-Assisted Recomps applies |
| `quiver` | slug, catalog id, page link, type, consoles, systems, the raw install hints, sync time |

`preferredExecutables` is kept in `quiver` but not used: those are Quiver's program names, often
with wildcards, and the launcher finds the program to start by itself.

### When RecompLauncher's catalog gets the game

Each time the catalog is read, an imported entry whose repository the catalog now lists is marked
`superseded_by` and hidden from the Catalog, and `merge_quiver` (lib.rs) moves everything over to
the native entry, using the same code as upstream's `merge_mine`: the installed copy where it is
(nothing is downloaded again), version, play time, favorites and other Library settings, mods, save
backups and Look. From then on the game installs, updates and sets up the RecompLauncher way (its
Easy Setup, boot profile and file patterns). With Data Hoarder Mode on, the game's releases folder
is renamed to the native entry's name so nothing is downloaded twice. It waits while the game is
running, downloading, moving or being set up, and when both entries are installed both stay.

### Filling in missing details (matched games)

For each RecompLauncher entry Quiver also lists (same repository), the sync keeps Quiver's details
(`fill` in `quiver-catalog.json`). When the catalog is read, a field the RecompLauncher entry leaves
empty is filled in from them: `game`, `origin`, `author`, `blurb`, `description`, `cover`, `hero`,
`logo`, `icon`, `tags`, `systems`. Where both have a value, RecompLauncher's stays: disagreements
are ignored. The repository, versions, release files and anything about installing are never
taken from Quiver. Filled fields are listed in the entry's `filled_from_quiver`.

### Standardized to RecompLauncher

Quiver's values are always put in RecompLauncher's words before they're used, both for filling in
and for imported games (`quiver::Vocab`):

- **Consoles**: matched against the catalog's `origins` by id *or* name, ignoring case, spaces and
  punctuation, so "N64", "n64" and "Nintendo 64" are all `n64`. Quiver's own ids that differ are
  mapped (`gcn`→`gc`, `nds`→`ds`, `og xbox`→`xbox`, `gen`→`genesis`, `32x`→`s32x`,
  `dreamcast`→`dc`, `ami`→`amiga`, `mobile`→`other`). A console the catalog doesn't know isn't used
  to fill a gap; an imported game keeps it and the catalog gets Quiver's name for it.
- **Tags**: a tag RecompLauncher's catalog already uses is written its way ("widescreen" and
  "high-fps" become "Widescreen" and "High FPS"); duplicates that differ only in spelling are
  dropped.
- **Dates**: YYYY-MM-DD, like the catalog.

### Keeping it current

- **Keep in Sync**: syncs again once a day, in the background, when the catalog is read.
- A game the last sync imported that Quiver no longer lists stays while it's in your Library
  (projects get taken down; Data Hoarder users want them kept). Others go.
- **Show Quiver Games in the Catalog** off hides them from the Catalog; ones in the Library stay.
- **Remove Them** takes the imported games out, except ones in the Library.

### Code

`src-tauri/src/quiver.rs` (fetching, matching, entries, filling in, standardizing; unit tests),
`merge_quiver`, `move_over` and the `quiver_*` commands in `src-tauri/src/lib.rs`, `ui/quiver.js`
(the Settings section, the badge and the card; app.js only calls `importGroup`, `importShown`,
`importBadge` and `importCard` when they exist, because upstream's tests keep other launchers'
names out of app.js), `tests/quiver-sync.test.cjs`.
