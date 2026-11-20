//! Add to Playnite: a game in Playnite's library, with its art, that starts the way the player's
//! desktop shortcuts do (straight to the game, or through the launcher when Settings → Shortcuts
//! Go Through the Launcher is on).
//!
//! Playnite has no command line or file to add a game from outside, and its library is a database
//! only Playnite itself should write. So the launcher puts a small Playnite add-on in Playnite's
//! Extensions folder (RecompLauncher Link, source in src-tauri/playnite/src) and hands it each game
//! as a request file in the add-on's own data folder. The add-on adds the game when Playnite starts,
//! and watches the folder while Playnite is open, so a game added from the launcher appears in a
//! second. Playnite loads add-ons when it starts: the first time, an open Playnite is restarted
//! (asked first, like Steam), or the game simply goes in the next time Playnite opens.
//!
//! Windows only: Playnite is a Windows program.

use crate::Error;
use serde_json::{json, Value};
use std::fs;
use std::path::{Path, PathBuf};

/// The add-on, as Playnite loads it: its manifest and its DLL (built with
/// tools/build-playnite-link.sh, deterministic, checked against its source by CI).
pub const MANIFEST: &str = include_str!("../playnite/extension.yaml");
pub const DLL: &[u8] = include_bytes!("../playnite/RecompLauncherLink.dll");
/// The add-on's folder name under Extensions, and its plugin Id (its data folder's name under
/// ExtensionsData). Both fixed: Playnite finds the add-on by them, and the add-on finds its games
/// again by GameId "recomplauncher:<id>".
pub const EXT_FOLDER: &str = "RecompLauncher_Link_458c5c75";
pub const PLUGIN_ID: &str = "458c5c75-a027-41d4-a666-b72ad5236af0";

/// Playnite on this PC: the folder its programs are in, the folder it keeps its library and add-ons
/// in, and its process when it's open.
#[derive(Debug, Clone)]
pub struct Playnite {
    pub program: PathBuf,
    pub data: PathBuf,
    pub pid: Option<u32>,
}

impl Playnite {
    pub fn ext_dir(&self) -> PathBuf {
        self.data.join("Extensions").join(EXT_FOLDER)
    }
    pub fn user_dir(&self) -> PathBuf {
        self.data.join("ExtensionsData").join(PLUGIN_ID)
    }
    pub fn queue(&self) -> PathBuf {
        self.user_dir().join("queue")
    }
    pub fn desktop_exe(&self) -> PathBuf {
        self.program.join("Playnite.DesktopApp.exe")
    }
    /// Is the add-on loaded in the Playnite that's open now? It writes Playnite's process id to
    /// link.json when it starts (and removes it when Playnite closes).
    pub fn link_loaded(&self) -> bool {
        let Some(pid) = self.pid else { return false };
        fs::read_to_string(self.user_dir().join("link.json")).ok().and_then(|s| serde_json::from_str::<Value>(&s).ok()).and_then(|v| v.get("pid").and_then(Value::as_u64)).map(|p| p == pid as u64).unwrap_or(false)
    }
}

/// Where Playnite keeps its things for a program folder. Playnite counts itself portable when its
/// folder has no uninstaller (unins000.exe): then everything is in that folder. Installed, it uses
/// %AppData%\Playnite.
pub fn data_dir(program: &Path, appdata: Option<&Path>) -> PathBuf {
    if program.join("unins000.exe").is_file() {
        if let Some(a) = appdata {
            return a.join("Playnite");
        }
    }
    program.to_path_buf()
}

/// Finds Playnite: the one that's open, else the one installed (its uninstall entry), else the
/// usual install folder (%LocalAppData%\Playnite).
pub fn find() -> Option<Playnite> {
    #[cfg(windows)]
    {
        let appdata = dirs::config_dir();
        let open = crate::win::processes().into_iter().find(|(_, n)| n.eq_ignore_ascii_case("Playnite.DesktopApp.exe") || n.eq_ignore_ascii_case("Playnite.FullscreenApp.exe"));
        let pid = open.as_ref().map(|(p, _)| *p);
        let program = open
            .and_then(|(p, _)| crate::win::process_file(p))
            .and_then(|f| f.parent().map(Path::to_path_buf))
            .or_else(installed_program)
            .or_else(|| dirs::data_local_dir().map(|d| d.join("Playnite")).filter(|d| d.join("Playnite.DesktopApp.exe").is_file()))?;
        let data = data_dir(&program, appdata.as_deref());
        Some(Playnite { program, data, pid })
    }
    #[cfg(not(windows))]
    {
        None
    }
}

/// Is Playnite on this PC? For the window's state, which is asked for often: looked up at most
/// once a minute.
pub fn present() -> bool {
    use std::sync::Mutex;
    use std::time::{Duration, Instant};
    static LAST: Mutex<Option<(Instant, bool)>> = Mutex::new(None);
    let mut last = LAST.lock().unwrap();
    if let Some((at, v)) = *last {
        if at.elapsed() < Duration::from_secs(60) {
            return v;
        }
    }
    let v = find().is_some();
    *last = Some((Instant::now(), v));
    v
}

#[cfg(windows)]
fn installed_program() -> Option<PathBuf> {
    use crate::win::{reg_str, reg_subkeys, HKCU, HKLM};
    const UNINSTALL: &str = r"Software\Microsoft\Windows\CurrentVersion\Uninstall";
    for root in [HKCU, HKLM] {
        for k in reg_subkeys(root, UNINSTALL) {
            let key = format!(r"{UNINSTALL}\{k}");
            let name = reg_str(root, &key, "DisplayName").unwrap_or_default();
            if !name.trim().eq_ignore_ascii_case("Playnite") && !name.starts_with("Playnite ") {
                continue;
            }
            if let Some(dir) = reg_str(root, &key, "InstallLocation").map(|d| PathBuf::from(d.trim().trim_matches('"'))) {
                if dir.join("Playnite.DesktopApp.exe").is_file() {
                    return Some(dir);
                }
            }
        }
    }
    None
}

/// Puts the add-on in Playnite's Extensions folder, or brings it up to date. A DLL Playnite has
/// open can't be replaced: the one there keeps working (the request format only grows), and the
/// new one goes in another time. True when it was missing before (Playnite has to start again to
/// load it).
pub fn install_link(p: &Playnite) -> Result<bool, Error> {
    let dir = p.ext_dir();
    let dll = dir.join("RecompLauncherLink.dll");
    let was_there = dll.is_file();
    fs::create_dir_all(&dir).map_err(|e| Error::Msg(format!("Couldn't put RecompLauncher's add-on in Playnite's Extensions folder: {e}")))?;
    if fs::read(dir.join("extension.yaml")).ok().as_deref() != Some(MANIFEST.as_bytes()) {
        let _ = fs::write(dir.join("extension.yaml"), MANIFEST);
    }
    if fs::read(&dll).ok().as_deref() != Some(DLL) {
        if let Err(e) = fs::write(&dll, DLL) {
            if !was_there {
                return Err(Error::Msg(format!("Couldn't put RecompLauncher's add-on in Playnite's Extensions folder: {e}")));
            }
        }
    }
    Ok(!was_there)
}

/// How Playnite starts the game.
pub struct Start {
    pub exe: PathBuf,
    pub args: String,
    /// Through the launcher: Playnite counts play time while a program from the game's folder runs
    /// (the launcher's own process can hand the game to an open launcher and quit at once).
    pub through_launcher: bool,
}

/// The request file the add-on reads (see RecompLauncherLink.cs, class Request).
pub fn request(id: &str, name: &str, start: &Start, folder: &Path, pics: &[(&str, String)]) -> Value {
    let mut r = json!({
        "v": 1,
        "key": format!("recomplauncher:{id}"),
        "name": name,
        "path": start.exe.to_string_lossy(),
        "args": start.args,
        "workdir": start.exe.parent().map(|d| d.to_string_lossy().to_string()).unwrap_or_default(),
        "install_dir": folder.to_string_lossy(),
        "tracking": if start.through_launcher { "directory" } else { "default" },
        "tracking_path": folder.to_string_lossy(),
    });
    for (k, f) in pics {
        r[*k] = json!(f);
    }
    r
}

/// A file name for the game's request (ids can hold characters a file name can't).
pub fn file_key(id: &str) -> String {
    id.chars().map(|c| if c.is_ascii_alphanumeric() || c == '-' || c == '_' || c == '.' { c } else { '_' }).collect()
}

/// Writes the game's pictures and then its request into the add-on's queue. The request goes in
/// last, under a temporary name first, so the add-on never reads half of one.
pub fn queue(p: &Playnite, id: &str, name: &str, start: &Start, folder: &Path, art: Vec<(&'static str, Vec<u8>, String)>) -> Result<PathBuf, Error> {
    let q = p.queue();
    fs::create_dir_all(&q).map_err(|e| Error::Msg(format!("Couldn't write to Playnite's add-on folder: {e}")))?;
    let key = file_key(id);
    let mut pics = vec![];
    for (kind, bytes, ext) in art {
        if bytes.is_empty() {
            continue;
        }
        let f = format!("{key}.{kind}.{ext}");
        if fs::write(q.join(&f), &bytes).is_ok() {
            pics.push((kind, f));
        }
    }
    let body = serde_json::to_vec_pretty(&request(id, name, start, folder, &pics)).unwrap_or_default();
    let tmp = q.join(format!("{key}.json.part"));
    let file = q.join(format!("{key}.json"));
    fs::write(&tmp, body)?;
    fs::rename(&tmp, &file)?;
    Ok(file)
}

/// Closes Playnite (its own --shutdown), waiting up to 30 seconds.
pub async fn close(p: &Playnite) -> Result<(), Error> {
    let mut c = std::process::Command::new(p.desktop_exe());
    crate::sysenv::clean_std(&mut c);
    c.arg("--shutdown");
    if let Ok(mut ch) = c.spawn() {
        std::thread::spawn(move || {
            let _ = ch.wait();
        });
    }
    for _ in 0..60 {
        tokio::time::sleep(std::time::Duration::from_millis(500)).await;
        if find().map(|n| n.pid.is_none()).unwrap_or(true) {
            return Ok(());
        }
    }
    Err(Error::Msg("Playnite didn't close. Close it yourself, and the game goes in the next time it opens.".into()))
}

pub fn open(p: &Playnite) {
    let mut c = std::process::Command::new(p.desktop_exe());
    crate::sysenv::clean_std(&mut c);
    if let Ok(mut ch) = c.spawn() {
        std::thread::spawn(move || {
            let _ = ch.wait();
        });
    }
}

/// Waits up to `secs` for the add-on to take the request (it deletes the file once the game is in).
pub async fn wait_taken(file: &Path, secs: u64) -> bool {
    for _ in 0..secs * 4 {
        if !file.exists() {
            return true;
        }
        tokio::time::sleep(std::time::Duration::from_millis(250)).await;
    }
    !file.exists()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn portable_playnite_keeps_everything_in_its_folder() {
        let t = std::env::temp_dir().join(format!("rl-playnite-{}", std::process::id()));
        let _ = fs::remove_dir_all(&t);
        fs::create_dir_all(&t).unwrap();
        let appdata = Path::new("/appdata");
        assert_eq!(data_dir(&t, Some(appdata)), t);
        fs::write(t.join("unins000.exe"), b"").unwrap();
        assert_eq!(data_dir(&t, Some(appdata)), appdata.join("Playnite"));
        let p = Playnite { program: t.clone(), data: t.clone(), pid: None };
        assert_eq!(p.ext_dir(), t.join("Extensions").join(EXT_FOLDER));
        assert_eq!(p.queue(), t.join("ExtensionsData").join(PLUGIN_ID).join("queue"));
        let _ = fs::remove_dir_all(&t);
    }

    #[test]
    fn the_manifest_and_the_add_on_agree_on_their_ids() {
        assert!(MANIFEST.lines().any(|l| l.trim() == format!("Id: {EXT_FOLDER}")));
        assert!(MANIFEST.contains("Module: RecompLauncherLink.dll"));
        let src = include_str!("../playnite/src/RecompLauncherLink.cs");
        assert!(src.contains(&format!("Guid.Parse(\"{PLUGIN_ID}\")")));
        assert!(DLL.starts_with(b"MZ"));
    }

    #[test]
    fn request_starts_the_game_the_way_shortcuts_do() {
        let folder = Path::new("C:/Games/Zelda");
        let direct = Start { exe: folder.join("zelda.exe"), args: "--fullscreen".into(), through_launcher: false };
        let r = request("zelda64", "Zelda", &direct, folder, &[("cover", "zelda64.cover.jpg".into())]);
        assert_eq!(r["key"], "recomplauncher:zelda64");
        assert_eq!(r["tracking"], "default");
        assert_eq!(r["args"], "--fullscreen");
        assert_eq!(r["cover"], "zelda64.cover.jpg");
        let via = Start { exe: PathBuf::from("C:/RL/RecompLauncher.exe"), args: "--play zelda64".into(), through_launcher: true };
        let r = request("zelda64", "Zelda", &via, folder, &[]);
        assert_eq!(r["tracking"], "directory");
        assert_eq!(r["tracking_path"], folder.to_string_lossy().as_ref());
        assert_eq!(file_key("sos/ps5:a b"), "sos_ps5_a_b");
    }

    #[test]
    fn queue_writes_pictures_then_the_request() {
        let t = std::env::temp_dir().join(format!("rl-playnite-q-{}", std::process::id()));
        let _ = fs::remove_dir_all(&t);
        let p = Playnite { program: t.clone(), data: t.clone(), pid: None };
        let start = Start { exe: t.join("game.exe"), args: String::new(), through_launcher: false };
        let f = queue(&p, "g1", "Game", &start, &t, vec![("cover", b"jpg".to_vec(), "jpg".into()), ("background", vec![], "jpg".into())]).unwrap();
        let r: Value = serde_json::from_slice(&fs::read(&f).unwrap()).unwrap();
        assert_eq!(r["cover"], "g1.cover.jpg");
        assert!(r.get("background").is_none());
        assert!(p.queue().join("g1.cover.jpg").is_file());
        assert!(!p.queue().join("g1.json.part").exists());
        let _ = fs::remove_dir_all(&t);
    }
}
