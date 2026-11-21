//! Quiver Catalog Sync (fork feature): the games Quiver Launcher's public catalog lists that
//! RecompLauncher's own catalog doesn't, added to the Catalog as entries of their own.
//!
//! - Read from quiverlauncher.com's public API (`/release-status` names every app's repository,
//!   `/apps` has its name, art and description). Nothing is sent about the player.
//! - Matched by repository only, never by title: `owner/name` on GitHub, `gitlab:group/project`,
//!   `forgejo:server/owner/name` and `sourceforge:name`, all compared in lowercase with any `.git`
//!   and trailing slash taken off. A repository RecompLauncher's catalog (or the player's own
//!   games added by link) already has is never imported.
//! - Kept in `quiver-catalog.json` beside library.json and joined to the catalog in
//!   `catalog::with_mine`, so every feature that reads the catalog (Install, Data Hoarder, Folder
//!   Structure, Add to Playnite) sees them like any other game.
//! - When RecompLauncher's own catalog later lists the same repository, the entry is marked
//!   `superseded_by` and lib.rs (merge_quiver) moves the player's copy, library state and settings
//!   over to the native entry, which installs and updates the RecompLauncher way from then on.

use crate::Error;
use serde_json::{json, Value};
use std::collections::{HashMap, HashSet};
use std::path::{Path, PathBuf};
use std::sync::Mutex;
use std::time::{Duration, SystemTime, UNIX_EPOCH};

/// The catalog's public API, and the same API on its host's own address for when the first one
/// can't be reached (Quiver Launcher does the same).
pub const API: &str = "https://api.quiverlauncher.com/api/v1";
pub const FALLBACK_API: &str = "https://famous-wildebeest-660.convex.site/api/v1";

/// Prefix of every imported entry's id, so they never clash with the catalog's own ids.
pub const ID_PREFIX: &str = "qv-";

/// How often "Keep in Sync" reads the catalog again.
pub const AUTO_EVERY: u64 = 24 * 3600;

/// Held while the file is read, changed and written back.
pub static TURN: Mutex<()> = Mutex::new(());

/// Systems RecompLauncher runs on: an app only for phones isn't imported.
const DESKTOP: &[&str] = &["windows", "linux", "macos"];

pub fn file(dir: &Path) -> PathBuf {
    dir.join("quiver-catalog.json")
}

/// What was saved: `{ "synced": secs, "auto": bool, "include_tools": bool, "apps": [...], "stats": {...} }`.
pub fn load(dir: &Path) -> Value {
    std::fs::read(file(dir)).ok().and_then(|b| serde_json::from_slice::<Value>(&b).ok()).filter(Value::is_object).unwrap_or_else(|| json!({}))
}

/// Written to a new file that then takes the old one's place, so a launcher closed halfway never
/// leaves it cut short.
pub fn save(dir: &Path, v: &Value) -> std::io::Result<()> {
    let f = file(dir);
    let tmp = f.with_extension(format!("json.{}.tmp", std::process::id()));
    std::fs::write(&tmp, serde_json::to_vec_pretty(v).unwrap_or_default())?;
    std::fs::rename(&tmp, &f)?;
    *CACHE.lock().unwrap_or_else(|e| e.into_inner()) = None;
    Ok(())
}

/// The imported entries, read again only when the file changed: the catalog is put together many
/// times as the window opens.
static CACHE: Mutex<Option<(PathBuf, SystemTime, u64, Value)>> = Mutex::new(None);

fn cached(dir: &Path) -> Value {
    let f = file(dir);
    let Some((time, len)) = std::fs::metadata(&f).ok().and_then(|m| Some((m.modified().ok()?, m.len()))) else { return json!({}) };
    if let Some((p, t, l, v)) = CACHE.lock().unwrap_or_else(|e| e.into_inner()).as_ref() {
        if *p == f && *t == time && *l == len {
            return v.clone();
        }
    }
    let v = load(dir);
    *CACHE.lock().unwrap_or_else(|e| e.into_inner()) = Some((f, time, len, v.clone()));
    v
}

pub fn entries(dir: &Path) -> Vec<Value> {
    cached(dir).get("apps").and_then(Value::as_array).cloned().unwrap_or_default()
}

pub fn is_quiver(a: &Value) -> bool {
    a.get("quiver").map(Value::is_object).unwrap_or(false)
}

/// A repository as compared: lowercase, no `.git`, no trailing slash.
pub fn match_key(repo: &str) -> String {
    let r = repo.trim().trim_end_matches('/');
    let r = r.strip_suffix(".git").unwrap_or(r);
    r.to_ascii_lowercase()
}

/// A Quiver app's repository the launcher's way, or None when it's somewhere the launcher can't
/// download from (a self-hosted GitLab or GitHub Enterprise, a release feed, a plain web page).
pub fn repo_key(provider: &str, host: &str, repository: &str) -> Option<String> {
    let r = repository.trim().trim_end_matches('/');
    let r = r.strip_suffix(".git").unwrap_or(r);
    let host = host.trim().to_ascii_lowercase();
    let repo = match provider {
        "github" if host.is_empty() || host == "github.com" => crate::catalog::repo_from_link(r)?,
        "gitlab" if host.is_empty() || host == "gitlab.com" => format!("gitlab:{}", r.trim_start_matches("https://gitlab.com/")),
        "gitea" | "forgejo" if !host.is_empty() => format!("forgejo:{host}/{r}"),
        // A link to the project's page: SourceForge is the one site of those the launcher reads.
        "manual" => crate::catalog::repo_from_link(r).filter(|x| crate::catalog::is_sourceforge(x))?,
        _ => return None,
    };
    crate::catalog::repo_ok(&repo).then_some(repo)
}

/// Quiver's console ids, where they differ from the catalog's own.
fn origin_of(id: &str) -> String {
    let id = id.trim().to_ascii_lowercase();
    let mapped = match id.as_str() {
        "gcn" => "gc",
        "nds" => "ds",
        "og xbox" => "xbox",
        "gen" => "genesis",
        "32x" => "s32x",
        "dreamcast" => "dc",
        "ami" => "amiga",
        "mobile" => "other",
        _ => id.as_str(),
    };
    mapped.to_string()
}

/// Quiver's console names, for ids the catalog has no name for.
pub fn console_names() -> HashMap<&'static str, &'static str> {
    HashMap::from([
        ("vita", "PlayStation Vita"),
        ("xone", "Xbox One"),
        ("sms", "Master System"),
        ("segacd", "Sega CD"),
        ("3do", "3DO"),
    ])
}

pub fn day_of(ms: Option<i64>) -> String {
    let Some(ms) = ms.filter(|m| *m > 0) else { return String::new() };
    let days = ms / 86_400_000;
    // Civil date from days since 1970-01-01 (Howard Hinnant's algorithm).
    let z = days + 719_468;
    let era = z.div_euclid(146_097);
    let doe = z - era * 146_097;
    let yoe = (doe - doe / 1460 + doe / 36_524 - doe / 146_096) / 365;
    let doy = doe - (365 * yoe + yoe / 4 - yoe / 100);
    let mp = (5 * doy + 2) / 153;
    let d = doy - (153 * mp + 2) / 5 + 1;
    let m = if mp < 10 { mp + 3 } else { mp - 9 };
    let y = yoe + era * 400 + if m <= 2 { 1 } else { 0 };
    format!("{y:04}-{m:02}-{d:02}")
}

fn s<'a>(v: &'a Value, k: &str) -> &'a str {
    v.get(k).and_then(Value::as_str).unwrap_or("")
}

fn https_or_empty(u: &str) -> String {
    if u.starts_with("https://") && !u.contains(char::is_whitespace) {
        u.to_string()
    } else {
        String::new()
    }
}

/// An id from a Quiver slug: lowercase letters, digits and dashes.
pub fn id_for(slug: &str) -> String {
    let body: String = slug.to_ascii_lowercase().chars().map(|c| if c.is_ascii_alphanumeric() { c } else { '-' }).collect();
    let body = body.trim_matches('-');
    let body: String = body.chars().take(90).collect();
    format!("{ID_PREFIX}{body}")
}

/// One Quiver app as a catalog entry. `status` is its `/release-status` row (its repository).
pub fn entry_from(app: &Value, status: &Value, now: u64) -> Option<Value> {
    build(app, status, now, true)
}

/// `desktop`: None for an app with no build for a system RecompLauncher runs on (an import);
/// without it, any app (filling in a matched game's details).
fn build(app: &Value, status: &Value, now: u64, desktop: bool) -> Option<Value> {
    let repo = repo_key(s(status, "provider"), s(status, "host"), s(status, "repository"))?;
    let slug = s(app, "slug");
    if slug.is_empty() {
        return None;
    }
    let systems: Vec<&str> = app.get("supportedOS").and_then(Value::as_array).map(|l| l.iter().filter_map(Value::as_str).filter(|o| DESKTOP.contains(o)).collect()).unwrap_or_default();
    if desktop && systems.is_empty() {
        return None;
    }
    let project = s(app, "projectName");
    let title = app.get("games").and_then(Value::as_array).and_then(|g| g.first()).map(|g| s(g, "title")).unwrap_or("");
    let name = if project.is_empty() { s(app, "name") } else { project };
    let game = if title.is_empty() { s(app, "name") } else { title };
    let console = app.get("basedOn").map(|b| s(b, "console")).filter(|c| !c.is_empty()).or_else(|| app.get("consoles").and_then(Value::as_array).and_then(|l| l.first()).and_then(Value::as_str)).unwrap_or("");
    let art = app.get("libraryArt").cloned().unwrap_or(Value::Null);
    let owner = crate::catalog::host_path(&repo).split('/').next().unwrap_or("").to_string();
    let author = app.get("developer").map(|d| s(d, "name")).filter(|n| !n.is_empty()).map(str::to_string).unwrap_or(owner);
    let description = s(app, "description");
    let verified = app.get("verified").cloned().unwrap_or(Value::Null);
    let tag = s(&verified, "version");
    let released_ms = verified.get("releasedAt").and_then(Value::as_i64).or_else(|| app.get("lastReleaseAt").and_then(Value::as_i64));
    let tags: Vec<Value> = app.get("tags").and_then(Value::as_array).map(|l| l.iter().filter_map(Value::as_str).filter(|t| !t.trim().is_empty()).take(12).map(|t| json!(t.trim())).collect()).unwrap_or_default();
    let launcher = app.get("launcher").cloned().unwrap_or(Value::Null);
    let filter = s(&launcher, "releaseAssetFilter").trim().to_string();
    let mut e = json!({
        "id": id_for(slug),
        "name": name.trim(),
        "game": game.trim(),
        "origin": origin_of(console),
        "repo": repo,
        "author": author,
        "blurb": description.chars().take(600).collect::<String>(),
        "description": description,
        "cover": https_or_empty(s(&art, "capsule")),
        "hero": https_or_empty(if s(&art, "hero").is_empty() { s(&art, "header") } else { s(&art, "hero") }),
        "logo": https_or_empty(s(&art, "logo")),
        "icon": https_or_empty(s(app, "artwork")),
        "systems": systems,
        "tags": tags,
        "added": day_of(app.get("addedAt").and_then(Value::as_i64)),
        "released": day_of(released_ms),
        "tag": tag,
        "version": tag.trim_start_matches(['v', 'V']),
        "quiver": {
            "slug": slug,
            "catalog_id": s(app, "catalogId"),
            "app_id": s(app, "id"),
            "url": format!("https://quiverlauncher.com/apps/{slug}"),
            "project_type": s(app, "projectType"),
            "consoles": app.get("consoles").cloned().unwrap_or(json!([])),
            "supported_os": app.get("supportedOS").cloned().unwrap_or(json!([])),
            "release_asset_filter": filter,
            "preferred_executables": launcher.get("preferredExecutables").cloned().unwrap_or(Value::Null),
            "ai_level": s(app, "aiLevel"),
            "recommended": app.get("recommended").cloned().unwrap_or(Value::Null),
            "reviews": app.get("reviewCount").cloned().unwrap_or(Value::Null),
            "synced": now,
        },
    });
    if crate::catalog::is_forgejo(&repo) {
        e["host"] = json!("forgejo");
    } else if crate::catalog::is_gitlab(&repo) {
        e["host"] = json!("gitlab");
    }
    // Quiver's file filter is a piece of the file's name, any case: as the catalog's own file
    // pattern, for each system it has a build for.
    if !filter.is_empty() && !filter.contains(['/', '\\']) {
        let pattern = format!("*{filter}*");
        e["assets"] = Value::Object(systems.iter().map(|o| (o.to_string(), json!(pattern))).collect());
    }
    let ai = s(app, "aiLevel");
    if !ai.is_empty() && ai != "none" {
        // The catalog's own shape ({ by, uses, note }), so the AI card and Hide AI work on it.
        e["ai"] = json!({ "by": "quiver", "level": ai, "uses": ["code"], "note": format!("Quiver's catalog lists it as AI-{ai}.") });
    }
    Some(e)
}

/// What a sync found, for the Settings section.
#[derive(Debug, Default, Clone, PartialEq, serde::Serialize)]
pub struct Stats {
    /// Apps in Quiver's catalog.
    pub quiver: usize,
    /// Already in RecompLauncher's catalog (same repository).
    pub matched: usize,
    /// Already one of the player's own games added by link.
    pub yours: usize,
    /// Imported.
    pub imported: usize,
    /// Only for phones (or no system RecompLauncher runs on).
    pub not_desktop: usize,
    /// Tools and emulators, left out unless asked for.
    pub tools: usize,
    /// On a site the launcher can't download from.
    pub unsupported: usize,
    /// A second Quiver app from a repository already imported.
    pub same_repo: usize,
}

/// The entries to keep from Quiver's apps and their repositories. `known` holds the repositories
/// (match_key) RecompLauncher's catalog lists; `yours` the player's own games' repositories;
/// `before` the entries a previous sync made, so an app keeps its id when Quiver renames it.
pub fn import(apps: &[Value], status: &[Value], known: &HashSet<String>, yours: &HashSet<String>, before: &[Value], include_tools: bool, now: u64) -> (Vec<Value>, Stats) {
    let by_id: HashMap<&str, &Value> = status.iter().map(|r| (s(r, "id"), r)).collect();
    let old_ids: HashMap<String, String> = before.iter().filter_map(|e| Some((s(e.get("quiver")?, "catalog_id").to_string(), s(e, "id").to_string()))).filter(|(c, _)| !c.is_empty()).collect();
    let mut st = Stats { quiver: apps.len(), ..Default::default() };
    let mut seen_repos: HashSet<String> = HashSet::new();
    let mut seen_ids: HashSet<String> = HashSet::new();
    let mut out = vec![];
    for a in apps {
        let Some(row) = by_id.get(s(a, "id")) else {
            st.unsupported += 1;
            continue;
        };
        let Some(repo) = repo_key(s(row, "provider"), s(row, "host"), s(row, "repository")) else {
            st.unsupported += 1;
            continue;
        };
        let key = match_key(&repo);
        if known.contains(&key) {
            st.matched += 1;
            continue;
        }
        if yours.contains(&key) {
            st.yours += 1;
            continue;
        }
        let kind = s(a, "projectType");
        if !include_tools && matches!(kind, "tool" | "emulator") {
            st.tools += 1;
            continue;
        }
        let Some(mut e) = entry_from(a, row, now) else {
            st.not_desktop += 1;
            continue;
        };
        // One entry per repository: a second app from the same one (two games in one project)
        // would install the same files.
        if !seen_repos.insert(key) {
            st.same_repo += 1;
            continue;
        }
        if let Some(old) = old_ids.get(s(a, "catalogId")) {
            e["id"] = json!(old);
        }
        let id = s(&e, "id").to_string();
        if !seen_ids.insert(id.clone()) {
            e["id"] = json!(format!("{id}-{}", seen_ids.len()));
            seen_ids.insert(s(&e, "id").to_string());
        }
        out.push(e);
    }
    st.imported = out.len();
    (out, st)
}

/// The catalog's own repositories (not the player's, not Quiver's), as compared.
pub fn native_repos(cat: &Value) -> HashSet<String> {
    cat.get("apps")
        .and_then(Value::as_array)
        .map(|l| l.iter().filter(|a| !a.get("custom").and_then(Value::as_bool).unwrap_or(false) && !is_quiver(a)).filter_map(|a| a.get("repo").and_then(Value::as_str)).map(match_key).collect())
        .unwrap_or_default()
}

/// The catalog with the imported entries added after it. One whose repository the catalog now has
/// itself is marked `superseded_by` (the catalog's id): it stays so a copy the player has keeps its
/// page until merge_quiver moves it over, but the Catalog doesn't show it twice.
pub fn with_quiver(dir: &Path, mut cat: Value) -> Value {
    let saved = cached(dir);
    let list: Vec<Value> = saved.get("apps").and_then(Value::as_array).cloned().unwrap_or_default();
    let fill = saved.get("fill").and_then(Value::as_object).cloned().unwrap_or_default();
    if list.is_empty() && fill.is_empty() {
        return cat;
    }
    if !cat.get("apps").map(Value::is_array).unwrap_or(false) {
        return cat;
    }
    let vocab = Vocab::of(&cat);
    // Matched games: what RecompLauncher's own entry leaves empty, from Quiver (fill_gaps).
    for a in cat["apps"].as_array_mut().unwrap() {
        let Some(id) = a.get("id").and_then(Value::as_str).map(str::to_string) else { continue };
        if let Some(q) = fill.get(&id) {
            fill_gaps(a, q, &vocab);
        }
    }
    let native: HashMap<String, String> = cat["apps"]
        .as_array()
        .unwrap()
        .iter()
        .filter(|a| !a.get("custom").and_then(Value::as_bool).unwrap_or(false) && !is_quiver(a))
        .filter_map(|a| Some((match_key(a.get("repo")?.as_str()?), a.get("id")?.as_str()?.to_string())))
        .collect();
    let ids: HashSet<String> = cat["apps"].as_array().unwrap().iter().filter_map(|a| a.get("id").and_then(Value::as_str).map(str::to_string)).collect();
    // Consoles the catalog has no name for get Quiver's, so the Catalog's filters can show them.
    let mut origins: HashSet<String> = cat.get("origins").and_then(Value::as_array).map(|l| l.iter().filter_map(|o| o.get("id").and_then(Value::as_str).map(str::to_string)).collect()).unwrap_or_default();
    let names = console_names();
    let mut new_origins = vec![];
    let apps = cat["apps"].as_array_mut().unwrap();
    for mut e in list {
        let id = s(&e, "id").to_string();
        if id.is_empty() || ids.contains(&id) {
            continue;
        }
        if let Some(n) = native.get(&match_key(s(&e, "repo"))) {
            e["superseded_by"] = json!(n);
        }
        vocab.standardize(&mut e);
        let o = s(&e, "origin").to_string();
        if let Some(name) = names.get(o.as_str()) {
            if origins.insert(o.clone()) {
                new_origins.push(json!({ "id": o, "name": name }));
            }
        }
        apps.push(e);
    }
    if !new_origins.is_empty() {
        if let Some(l) = cat.get_mut("origins").and_then(Value::as_array_mut) {
            l.extend(new_origins);
        }
    }
    cat
}

// ---------- filling in a matched game's missing details ----------

/// The details Quiver can fill in on a RecompLauncher entry, when the entry has none of its own.
/// Never the repository, the version, the release files or anything about installing: those stay
/// RecompLauncher's.
pub const FILLABLE: &[&str] = &["game", "origin", "author", "blurb", "description", "cover", "hero", "logo", "icon", "tags", "systems"];

/// RecompLauncher's own words for things, read from its catalog: its console ids (and names), and
/// its tags as it writes them. Quiver's values are put in these words before they're used, so the
/// catalog never has two spellings of one thing ("N64" and "Nintendo 64", "widescreen" and
/// "Widescreen").
pub struct Vocab {
    origins: HashMap<String, String>,
    tags: HashMap<String, String>,
}

/// A word as compared: lowercase letters and digits only.
fn fold(s: &str) -> String {
    s.chars().filter(|c| c.is_alphanumeric()).flat_map(char::to_lowercase).collect()
}

impl Vocab {
    pub fn of(cat: &Value) -> Vocab {
        let mut origins = HashMap::new();
        for o in cat.get("origins").and_then(Value::as_array).into_iter().flatten() {
            let id = s(o, "id");
            if id.is_empty() {
                continue;
            }
            origins.insert(fold(id), id.to_string());
            origins.entry(fold(s(o, "name"))).or_insert_with(|| id.to_string());
        }
        // Quiver's ids and names for consoles the catalog calls something else.
        for (q, rl) in [("gcn", "gc"), ("nds", "ds"), ("ogxbox", "xbox"), ("gen", "genesis"), ("segagenesismegadrive", "genesis"), ("megadrive", "genesis"), ("32x", "s32x"), ("sega32x", "s32x"), ("dreamcast", "dc"), ("segadreamcast", "dc"), ("ami", "amiga"), ("commodoreamiga", "amiga"), ("supernintendoentertainmentsystem", "snes"), ("nintendoentertainmentsystem", "nes"), ("playstationportable", "psp"), ("segasaturn", "saturn"), ("segagamegear", "gg"), ("philipscdi", "cdi"), ("mobile", "other")] {
            if origins.values().any(|v| v == rl) {
                origins.entry(q.to_string()).or_insert_with(|| rl.to_string());
            }
        }
        let mut tags = HashMap::new();
        for a in cat.get("apps").and_then(Value::as_array).into_iter().flatten() {
            if a.get("custom").and_then(Value::as_bool).unwrap_or(false) || is_quiver(a) {
                continue;
            }
            for t in a.get("tags").and_then(Value::as_array).into_iter().flatten().filter_map(Value::as_str) {
                tags.entry(fold(t)).or_insert_with(|| t.to_string());
            }
        }
        Vocab { origins, tags }
    }

    /// A console the catalog's way: its id when it knows the console by any of its ids or names.
    pub fn origin(&self, v: &str) -> Option<String> {
        self.origins.get(&fold(v)).cloned()
    }

    /// A tag the catalog's way when it uses it already, else as Quiver wrote it.
    pub fn tag(&self, v: &str) -> String {
        self.tags.get(&fold(v)).cloned().unwrap_or_else(|| v.trim().to_string())
    }

    /// An imported entry in the catalog's words.
    pub fn standardize(&self, e: &mut Value) {
        if let Some(o) = self.origin(s(e, "origin")) {
            e["origin"] = json!(o);
        }
        if let Some(l) = e.get("tags").and_then(Value::as_array) {
            let mut seen = HashSet::new();
            let tags: Vec<Value> = l.iter().filter_map(Value::as_str).map(|t| self.tag(t)).filter(|t| seen.insert(fold(t))).map(Value::String).collect();
            e["tags"] = Value::Array(tags);
        }
    }
}

fn empty(v: Option<&Value>) -> bool {
    match v {
        None | Some(Value::Null) => true,
        Some(Value::String(s)) => s.trim().is_empty(),
        Some(Value::Array(a)) => a.is_empty(),
        Some(Value::Object(o)) => o.is_empty(),
        _ => false,
    }
}

/// Fills in what a RecompLauncher entry leaves empty from a matched Quiver app's details (`q`, as
/// an entry), in RecompLauncher's words. Where both have a value, RecompLauncher's stays: a
/// disagreement is ignored. A console Quiver names that the catalog doesn't know isn't filled in.
/// The fields filled are listed in `filled_from_quiver`.
pub fn fill_gaps(a: &mut Value, q: &Value, vocab: &Vocab) {
    let mut filled = vec![];
    for k in FILLABLE {
        if !empty(a.get(*k)) || empty(q.get(*k)) {
            continue;
        }
        let v = match *k {
            "origin" => match vocab.origin(s(q, "origin")) {
                Some(o) => json!(o),
                None => continue,
            },
            "tags" => {
                let mut seen = HashSet::new();
                json!(q["tags"].as_array().unwrap().iter().filter_map(Value::as_str).map(|t| vocab.tag(t)).filter(|t| seen.insert(fold(t))).collect::<Vec<_>>())
            }
            _ => q[*k].clone(),
        };
        a[*k] = v;
        filled.push(json!(k));
    }
    if !filled.is_empty() {
        a["filled_from_quiver"] = Value::Array(filled);
    }
}

/// For each RecompLauncher entry Quiver lists too (the same repository), Quiver's details to fill
/// its gaps with: `{ native id: { field: value } }`, only the fields fill_gaps can use.
pub fn fills(apps: &[Value], status: &[Value], native: &HashMap<String, String>, now: u64) -> serde_json::Map<String, Value> {
    let by_id: HashMap<&str, &Value> = status.iter().map(|r| (s(r, "id"), r)).collect();
    let mut out = serde_json::Map::new();
    for a in apps {
        let Some(row) = by_id.get(s(a, "id")) else { continue };
        let Some(repo) = repo_key(s(row, "provider"), s(row, "host"), s(row, "repository")) else { continue };
        let Some(id) = native.get(&match_key(&repo)) else { continue };
        if out.contains_key(id) {
            continue;
        }
        let Some(e) = build(a, row, now, false) else { continue };
        let keep: serde_json::Map<String, Value> = FILLABLE.iter().filter(|k| !empty(e.get(**k))).map(|k| (k.to_string(), e[*k].clone())).collect();
        if !keep.is_empty() {
            out.insert(id.clone(), Value::Object(keep));
        }
    }
    out
}

/// The catalog's own entries by repository (match_key → id).
pub fn native_ids(cat: &Value) -> HashMap<String, String> {
    cat.get("apps")
        .and_then(Value::as_array)
        .into_iter()
        .flatten()
        .filter(|a| !a.get("custom").and_then(Value::as_bool).unwrap_or(false) && !is_quiver(a))
        .filter_map(|a| Some((match_key(a.get("repo")?.as_str()?), a.get("id")?.as_str()?.to_string())))
        .collect()
}

/// Every page of one of the API's lists.
async fn pages(http: &reqwest::Client, base: &str, path: &str) -> Result<Vec<Value>, Error> {
    let mut out = vec![];
    let mut cursor: Option<String> = None;
    // A guard against a list that never ends: far more pages than the catalog has.
    for _ in 0..200 {
        let mut url = format!("{base}{path}?limit=100");
        if let Some(c) = cursor.as_deref() {
            url.push_str("&cursor=");
            url.push_str(&urlencoding(c));
        }
        let r = http.get(&url).timeout(Duration::from_secs(30)).send().await.map_err(|e| Error::Net(e.to_string()))?;
        if !r.status().is_success() {
            return Err(Error::Net(format!("Quiver's catalog answered {}", r.status())));
        }
        let page: Value = r.json().await.map_err(|e| Error::Net(e.to_string()))?;
        out.extend(page.get("items").and_then(Value::as_array).cloned().unwrap_or_default());
        let done = page.get("isDone").and_then(Value::as_bool).unwrap_or(true);
        cursor = page.get("nextCursor").and_then(Value::as_str).map(str::to_string);
        if done || cursor.is_none() {
            return Ok(out);
        }
    }
    Ok(out)
}

fn urlencoding(s: &str) -> String {
    s.bytes().map(|b| if b.is_ascii_alphanumeric() || b"-_.~".contains(&b) { (b as char).to_string() } else { format!("%{b:02X}") }).collect()
}

/// Quiver's apps and each one's repository, from its own address or the fallback one.
pub async fn fetch(http: &reqwest::Client) -> Result<(Vec<Value>, Vec<Value>), Error> {
    let mut last = Error::Msg("Quiver's catalog couldn't be reached.".into());
    // RL_QUIVER_API: a local copy of the API, for testing.
    let local = std::env::var("RL_QUIVER_API").ok().filter(|s| !s.is_empty());
    let bases: Vec<String> = match local {
        Some(b) => vec![b.trim_end_matches('/').to_string()],
        None => vec![API.to_string(), FALLBACK_API.to_string()],
    };
    for base in bases.iter().map(String::as_str) {
        let got = async { Ok::<_, Error>((pages(http, base, "/apps").await?, pages(http, base, "/release-status").await?)) }.await;
        match got {
            Ok(v) if !v.0.is_empty() => return Ok(v),
            Ok(_) => last = Error::Msg("Quiver's catalog came back empty.".into()),
            Err(e) => last = e,
        }
    }
    Err(last)
}

pub fn now() -> u64 {
    SystemTime::now().duration_since(UNIX_EPOCH).map(|d| d.as_secs()).unwrap_or(0)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn repo_keys_follow_the_launchers_own_form() {
        assert_eq!(repo_key("github", "", "mstan/DrMarioNesRecomp").as_deref(), Some("mstan/DrMarioNesRecomp"));
        assert_eq!(repo_key("github", "", "https://github.com/Owner/Repo.git/").as_deref(), Some("Owner/Repo"));
        assert_eq!(repo_key("gitlab", "", "sonicdcer/Starfox64Recomp").as_deref(), Some("gitlab:sonicdcer/Starfox64Recomp"));
        assert_eq!(repo_key("gitea", "codeberg.org", "cande/cherry").as_deref(), Some("forgejo:codeberg.org/cande/cherry"));
        assert_eq!(repo_key("manual", "", "https://sourceforge.net/projects/hua3/").as_deref(), Some("sourceforge:hua3"));
    }

    #[test]
    fn hosts_the_launcher_cant_read_are_skipped() {
        assert_eq!(repo_key("github", "github.example.com", "a/b"), None);
        assert_eq!(repo_key("gitlab", "gitlab.example.com", "a/b"), None);
        assert_eq!(repo_key("feed", "", "https://example.com/feed.json"), None);
        assert_eq!(repo_key("manual", "", "https://example.com/game"), None);
    }

    #[test]
    fn match_key_ignores_case_dot_git_and_slash() {
        assert_eq!(match_key("Owner/Repo.git/"), "owner/repo");
        assert_eq!(match_key("owner/repo"), match_key("OWNER/REPO"));
    }

    #[test]
    fn days_from_milliseconds() {
        assert_eq!(day_of(Some(1_794_861_960_512)), "2026-11-16");
        assert_eq!(day_of(Some(0)), "");
        assert_eq!(day_of(None), "");
    }

    fn app(id: &str, slug: &str, kind: &str, os: &[&str]) -> Value {
        json!({ "id": id, "catalogId": format!("qcat_{id}"), "slug": slug, "name": "Game", "projectName": format!("{slug} Recomp"), "description": "A port.",
            "games": [{ "title": "The Game" }], "consoles": ["gcn"], "supportedOS": os, "projectType": kind, "tags": ["recomp"],
            "launcher": { "folderName": "x", "filesToAdd": [], "releaseAssetFilter": "win64" },
            "libraryArt": { "capsule": "https://cdn/c.png", "hero": "https://cdn/h.png", "logo": "http://insecure/l.png" },
            "verified": { "version": "v1.2.0", "releasedAt": 1_794_861_960_512u64 }, "addedAt": 1_794_861_960_512u64, "aiLevel": "assisted" })
    }

    fn row(id: &str, repo: &str) -> Value {
        json!({ "id": id, "provider": "github", "repository": repo })
    }

    #[test]
    fn an_entry_carries_the_apps_metadata() {
        let e = entry_from(&app("1", "game-recomp", "port", &["windows", "android"]), &row("1", "Dev/GameRecomp"), 7).unwrap();
        assert_eq!(e["id"], "qv-game-recomp");
        assert_eq!(e["name"], "game-recomp Recomp");
        assert_eq!(e["game"], "The Game");
        assert_eq!(e["origin"], "gc");
        assert_eq!(e["repo"], "Dev/GameRecomp");
        assert_eq!(e["systems"], json!(["windows"]));
        assert_eq!(e["cover"], "https://cdn/c.png");
        assert_eq!(e["logo"], "", "only https pictures");
        assert_eq!(e["assets"], json!({ "windows": "*win64*" }));
        assert_eq!(e["tag"], "v1.2.0");
        assert_eq!(e["version"], "1.2.0");
        assert_eq!(e["ai"]["level"], "assisted");
        assert_eq!(e["quiver"]["slug"], "game-recomp");
        assert_eq!(e["quiver"]["synced"], 7);
        assert!(e.get("custom").is_none(), "shown in the Catalog, unlike games added by link");
    }

    #[test]
    fn phone_only_apps_arent_entries() {
        assert!(entry_from(&app("1", "a", "port", &["android"]), &row("1", "a/b"), 0).is_none());
    }

    #[test]
    fn import_matches_by_repository_never_by_title() {
        let apps = vec![app("1", "same-title", "port", &["windows"]), app("2", "other", "port", &["windows"])];
        let status = vec![row("1", "Dev/Known.git"), row("2", "Dev/New")];
        // The catalog has a game with the same title but another repository: still imported.
        let known: HashSet<String> = ["dev/known".to_string()].into();
        let (out, st) = import(&apps, &status, &known, &HashSet::new(), &[], false, 0);
        assert_eq!(out.len(), 1);
        assert_eq!(out[0]["repo"], "Dev/New");
        assert_eq!(st.matched, 1);
        assert_eq!(st.imported, 1);
    }

    #[test]
    fn the_players_own_games_arent_imported_again() {
        let yours: HashSet<String> = ["dev/mine".to_string()].into();
        let (out, st) = import(&[app("1", "a", "port", &["windows"])], &[row("1", "dev/MINE")], &HashSet::new(), &yours, &[], false, 0);
        assert!(out.is_empty());
        assert_eq!(st.yours, 1);
    }

    #[test]
    fn tools_and_emulators_only_when_asked() {
        let apps = vec![app("1", "a", "tool", &["windows"]), app("2", "b", "emulator", &["windows"])];
        let status = vec![row("1", "x/a"), row("2", "x/b")];
        let (out, st) = import(&apps, &status, &HashSet::new(), &HashSet::new(), &[], false, 0);
        assert!(out.is_empty());
        assert_eq!(st.tools, 2);
        let (out, _) = import(&apps, &status, &HashSet::new(), &HashSet::new(), &[], true, 0);
        assert_eq!(out.len(), 2);
    }

    #[test]
    fn one_entry_per_repository() {
        let apps = vec![app("1", "fire-red", "port", &["windows"]), app("2", "leaf-green", "port", &["windows"])];
        let status = vec![row("1", "x/pokemon"), row("2", "X/Pokemon")];
        let (out, st) = import(&apps, &status, &HashSet::new(), &HashSet::new(), &[], false, 0);
        assert_eq!(out.len(), 1);
        assert_eq!(st.same_repo, 1);
    }

    #[test]
    fn an_app_keeps_its_id_when_quiver_renames_it() {
        let before = vec![json!({ "id": "qv-old-name", "quiver": { "catalog_id": "qcat_1" } })];
        let (out, _) = import(&[app("1", "new-name", "port", &["windows"])], &[row("1", "x/y")], &HashSet::new(), &HashSet::new(), &before, false, 0);
        assert_eq!(out[0]["id"], "qv-old-name");
    }

    fn vocab() -> Vocab {
        Vocab::of(&json!({ "origins": [{ "id": "n64", "name": "Nintendo 64" }, { "id": "gc", "name": "GameCube" }],
            "apps": [{ "id": "z", "repo": "a/z", "tags": ["Widescreen", "High FPS"] }] }))
    }

    #[test]
    fn quivers_words_become_recomplaunchers() {
        let v = vocab();
        assert_eq!(v.origin("N64").as_deref(), Some("n64"));
        assert_eq!(v.origin("Nintendo 64").as_deref(), Some("n64"));
        assert_eq!(v.origin("gcn").as_deref(), Some("gc"));
        assert_eq!(v.origin("vita"), None);
        assert_eq!(v.tag("widescreen"), "Widescreen");
        assert_eq!(v.tag("high-fps"), "High FPS");
        assert_eq!(v.tag("mods"), "mods");
    }

    #[test]
    fn a_matched_game_gets_only_its_missing_details() {
        let mut a = json!({ "id": "x", "repo": "a/x", "game": "Its Own Title", "origin": "", "cover": "", "tags": [] });
        let q = json!({ "game": "Quiver's Title", "origin": "N64", "cover": "https://cdn/c.png", "tags": ["widescreen", "Widescreen", "mods"], "author": "dev" });
        fill_gaps(&mut a, &q, &vocab());
        assert_eq!(a["game"], "Its Own Title", "a disagreement is ignored");
        assert_eq!(a["origin"], "n64", "standardized to the catalog's id");
        assert_eq!(a["cover"], "https://cdn/c.png");
        assert_eq!(a["tags"], json!(["Widescreen", "mods"]));
        assert_eq!(a["author"], "dev");
        assert_eq!(a["filled_from_quiver"], json!(["origin", "author", "cover", "tags"]));
    }

    #[test]
    fn a_console_the_catalog_doesnt_know_isnt_filled_in() {
        let mut a = json!({ "id": "x", "origin": "" });
        fill_gaps(&mut a, &json!({ "origin": "vita" }), &vocab());
        assert_eq!(a["origin"], "");
        assert!(a.get("filled_from_quiver").is_none());
    }

    #[test]
    fn fills_are_kept_for_matched_games_only() {
        let native: HashMap<String, String> = [("dev/known".to_string(), "known".to_string())].into();
        let apps = vec![app("1", "a", "port", &["android"]), app("2", "b", "port", &["windows"])];
        let out = fills(&apps, &[row("1", "Dev/Known"), row("2", "dev/new")], &native, 0);
        assert_eq!(out.len(), 1, "even a phone-only app's details fill in a matched game");
        assert_eq!(out["known"]["cover"], "https://cdn/c.png");
        assert!(out["known"].get("repo").is_none(), "never the repository");
    }

    #[test]
    fn with_quiver_fills_and_standardizes() {
        let d = std::env::temp_dir().join(format!("rl-quiver-fill-{}", std::process::id()));
        std::fs::create_dir_all(&d).unwrap();
        save(&d, &json!({ "fill": { "z": { "logo": "https://cdn/l.png", "origin": "Nintendo 64" } }, "apps": [{ "id": "qv-b", "repo": "dev/b", "origin": "gcn", "tags": ["widescreen"], "quiver": {} }] })).unwrap();
        let cat = with_quiver(&d, json!({ "origins": [{ "id": "n64", "name": "Nintendo 64" }, { "id": "gc", "name": "GameCube" }], "apps": [{ "id": "z", "repo": "a/z", "origin": "n64", "logo": "", "tags": ["Widescreen"] }] }));
        let apps = cat["apps"].as_array().unwrap();
        assert_eq!(apps[0]["logo"], "https://cdn/l.png");
        assert_eq!(apps[0]["filled_from_quiver"], json!(["logo"]));
        assert_eq!(apps[1]["origin"], "gc");
        assert_eq!(apps[1]["tags"], json!(["Widescreen"]));
        let _ = std::fs::remove_dir_all(&d);
    }

    #[test]
    fn with_quiver_marks_entries_the_catalog_now_has() {
        let d = std::env::temp_dir().join(format!("rl-quiver-{}", std::process::id()));
        std::fs::create_dir_all(&d).unwrap();
        save(&d, &json!({ "apps": [{ "id": "qv-a", "repo": "Dev/A", "origin": "vita", "quiver": {} }, { "id": "qv-b", "repo": "dev/b", "quiver": {} }] })).unwrap();
        let cat = with_quiver(&d, json!({ "apps": [{ "id": "a-native", "repo": "dev/a" }], "origins": [{ "id": "n64", "name": "Nintendo 64" }] }));
        let apps = cat["apps"].as_array().unwrap();
        assert_eq!(apps.len(), 3);
        assert_eq!(apps[1]["superseded_by"], "a-native");
        assert!(apps[2].get("superseded_by").is_none());
        assert!(cat["origins"].as_array().unwrap().iter().any(|o| o["id"] == "vita"));
        let _ = std::fs::remove_dir_all(&d);
    }
}
