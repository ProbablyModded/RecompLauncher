//! The player's own details for any game (fork): Edit Game Info on a catalog game, or one imported
//! from Quiver's catalog, saves what they change in `game-info.json` beside library.json, as
//! `{ id: { field: value } }`. It's laid over the catalog last (catalog::with_mine), so what the
//! player wrote wins over RecompLauncher's catalog and over anything filled in from Quiver, and
//! every feature that reads the catalog (Folder Structure, Data Hoarder, Add to Playnite, the
//! Catalog's filters) sees it. A catalog update never undoes it; Reset takes it off again.
//! Games the player added by link keep their details in their own entry (my-recomps.json).

use serde_json::{json, Map, Value};
use std::path::{Path, PathBuf};
use std::sync::Mutex;
use std::time::SystemTime;

/// What can be changed. Not the repository, versions or release files (how it installs), nor
/// the name and art, which Customize already changes for every game.
pub const FIELDS: &[&str] = &["game", "origin", "author", "genre", "first", "tags", "blurb", "website", "notes"];

pub static TURN: Mutex<()> = Mutex::new(());

pub fn file(dir: &Path) -> PathBuf {
    dir.join("game-info.json")
}

pub fn load(dir: &Path) -> Map<String, Value> {
    std::fs::read(file(dir)).ok().and_then(|b| serde_json::from_slice::<Value>(&b).ok()).and_then(|v| v.as_object().cloned()).unwrap_or_default()
}

static CACHE: Mutex<Option<(PathBuf, SystemTime, u64, Map<String, Value>)>> = Mutex::new(None);

fn cached(dir: &Path) -> Map<String, Value> {
    let f = file(dir);
    let Some((time, len)) = std::fs::metadata(&f).ok().and_then(|m| Some((m.modified().ok()?, m.len()))) else { return Map::new() };
    if let Some((p, t, l, v)) = CACHE.lock().unwrap_or_else(|e| e.into_inner()).as_ref() {
        if *p == f && *t == time && *l == len {
            return v.clone();
        }
    }
    let v = load(dir);
    *CACHE.lock().unwrap_or_else(|e| e.into_inner()) = Some((f, time, len, v.clone()));
    v
}

pub fn save(dir: &Path, all: &Map<String, Value>) -> std::io::Result<()> {
    let f = file(dir);
    let tmp = f.with_extension(format!("json.{}.tmp", std::process::id()));
    std::fs::write(&tmp, serde_json::to_vec_pretty(all).unwrap_or_default())?;
    std::fs::rename(&tmp, &f)?;
    *CACHE.lock().unwrap_or_else(|e| e.into_inner()) = None;
    Ok(())
}

/// One field's value as kept: trimmed and cut to size; tags a list; the website https only.
/// None: the field was left empty (the catalog's own value comes back).
pub fn clean(k: &str, v: &Value) -> Option<Value> {
    if k == "tags" {
        let words: Vec<String> = match v {
            Value::Array(l) => l.iter().filter_map(Value::as_str).map(str::to_string).collect(),
            Value::String(s) => s.split(',').map(str::to_string).collect(),
            _ => vec![],
        };
        let mut seen = std::collections::HashSet::new();
        let tags: Vec<Value> = words.iter().map(|w| w.trim().chars().take(40).collect::<String>()).filter(|w| !w.is_empty() && seen.insert(w.to_lowercase())).take(20).map(Value::String).collect();
        return (!tags.is_empty()).then_some(Value::Array(tags));
    }
    let s: String = v.as_str()?.trim().chars().take(if matches!(k, "blurb" | "notes") { 4000 } else { 300 }).collect();
    if s.is_empty() || (k == "website" && !s.starts_with("https://")) {
        return None;
    }
    Some(Value::String(s))
}

/// The player's changes for one game, from the form: a field sent empty goes back to the
/// catalog's value. Gives back what's kept for it.
pub fn set(all: &mut Map<String, Value>, id: &str, info: &Map<String, Value>) -> Value {
    let mut mine = all.get(id).and_then(Value::as_object).cloned().unwrap_or_default();
    for k in FIELDS {
        let Some(v) = info.get(*k) else { continue };
        match clean(k, v) {
            Some(v) => mine.insert(k.to_string(), v),
            None => mine.remove(*k),
        };
    }
    if mine.is_empty() {
        all.remove(id);
        return json!({});
    }
    all.insert(id.to_string(), Value::Object(mine.clone()));
    Value::Object(mine)
}

/// The catalog with the player's own details laid over it. The fields they changed are listed in
/// `edited`, and the catalog's own values kept in `catalog_info` so the form can show them.
pub fn with_own_info(dir: &Path, mut cat: Value) -> Value {
    let all = cached(dir);
    if all.is_empty() {
        return cat;
    }
    for a in cat.get_mut("apps").and_then(Value::as_array_mut).into_iter().flatten() {
        let Some(mine) = a.get("id").and_then(Value::as_str).and_then(|id| all.get(id)).and_then(Value::as_object).cloned() else { continue };
        let mut was = Map::new();
        let mut edited = vec![];
        for (k, v) in mine {
            if !FIELDS.contains(&k.as_str()) {
                continue;
            }
            was.insert(k.clone(), a.get(&k).cloned().unwrap_or(Value::Null));
            a[&k] = v;
            edited.push(Value::String(k));
        }
        if !edited.is_empty() {
            a["edited"] = Value::Array(edited);
            a["catalog_info"] = Value::Object(was);
        }
    }
    cat
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn an_empty_field_goes_back_to_the_catalogs_value() {
        let mut all = Map::new();
        set(&mut all, "z", json!({ "game": "Mine", "genre": "rpg" }).as_object().unwrap());
        let kept = set(&mut all, "z", json!({ "game": "" }).as_object().unwrap());
        assert_eq!(kept, json!({ "genre": "rpg" }));
        set(&mut all, "z", json!({ "genre": " " }).as_object().unwrap());
        assert!(!all.contains_key("z"));
    }

    #[test]
    fn install_details_cant_be_changed() {
        let mut all = Map::new();
        set(&mut all, "z", json!({ "repo": "evil/repo", "assets": { "windows": "x" }, "author": "Dev" }).as_object().unwrap());
        assert_eq!(all["z"], json!({ "author": "Dev" }));
    }

    #[test]
    fn tags_and_website_are_cleaned() {
        assert_eq!(clean("tags", &json!("Mods, mods , HD")), Some(json!(["Mods", "HD"])));
        assert_eq!(clean("website", &json!("http://insecure.example")), None);
        assert_eq!(clean("website", &json!("https://ok.example")), Some(json!("https://ok.example")));
    }

    #[test]
    fn the_players_details_win_and_are_marked() {
        let d = std::env::temp_dir().join(format!("rl-gameinfo-{}", std::process::id()));
        std::fs::create_dir_all(&d).unwrap();
        save(&d, json!({ "z": { "origin": "n64", "repo": "evil/x" } }).as_object().unwrap()).unwrap();
        let cat = with_own_info(&d, json!({ "apps": [{ "id": "z", "repo": "a/z", "origin": "pc" }, { "id": "y", "origin": "pc" }] }));
        assert_eq!(cat["apps"][0]["origin"], "n64");
        assert_eq!(cat["apps"][0]["repo"], "a/z");
        assert_eq!(cat["apps"][0]["edited"], json!(["origin"]));
        assert_eq!(cat["apps"][0]["catalog_info"]["origin"], "pc");
        assert!(cat["apps"][1].get("edited").is_none());
        let _ = std::fs::remove_dir_all(&d);
    }
}
