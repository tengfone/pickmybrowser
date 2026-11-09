//! Browser model, profile discovery and launching. OS-specific detection lives in `platform`.

use serde::{Deserialize, Serialize};
use std::path::{Path, PathBuf};
use std::process::Command;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum Engine {
    Chromium,
    Firefox,
    Safari,
    Other,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Profile {
    /// Value passed to the browser (Chromium dir name / Firefox profile name).
    pub id: String,
    pub name: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Browser {
    /// Stable id, e.g. "chrome", "firefox", or a normalised fallback.
    pub id: String,
    pub name: String,
    /// Family key used by the UI for the glyph ("chrome", "edge", ...).
    pub family: String,
    pub engine: Engine,
    /// Windows/Linux: executable (+ fixed args on Linux). macOS: path to the .app bundle.
    pub exec: String,
    #[serde(default)]
    pub exec_args: Vec<String>,
    #[serde(default)]
    pub profiles: Vec<Profile>,
    #[serde(default = "yes")]
    pub enabled: bool,
    /// Linux: `Icon=` from the .desktop entry (theme name or absolute path).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub icon: Option<String>,
}

fn yes() -> bool {
    true
}

struct Known {
    family: &'static str,
    name: &'static str,
    engine: Engine,
    /// Substrings (lowercase) matched against name / exe / bundle id.
    needles: &'static [&'static str],
    /// Chromium "User Data" dir or Firefox root, relative to the OS base dir.
    win: Option<&'static str>,
    mac: Option<&'static str>,
    linux: Option<&'static str>,
}

// Order matters: more specific needles first (e.g. "chrome beta" handled by "chrome").
const KNOWN: &[Known] = &[
    Known { family: "edge", name: "Microsoft Edge", engine: Engine::Chromium, needles: &["msedge", "microsoft edge", "com.microsoft.edgemac"],
        win: Some("Microsoft/Edge/User Data"), mac: Some("Microsoft Edge"), linux: Some("microsoft-edge") },
    Known { family: "brave", name: "Brave", engine: Engine::Chromium, needles: &["brave"],
        win: Some("BraveSoftware/Brave-Browser/User Data"), mac: Some("BraveSoftware/Brave-Browser"), linux: Some("BraveSoftware/Brave-Browser") },
    Known { family: "vivaldi", name: "Vivaldi", engine: Engine::Chromium, needles: &["vivaldi"],
        win: Some("Vivaldi/User Data"), mac: Some("Vivaldi"), linux: Some("vivaldi") },
    Known { family: "opera", name: "Opera", engine: Engine::Chromium, needles: &["opera"],
        win: None, mac: None, linux: None },
    Known { family: "arc", name: "Arc", engine: Engine::Chromium, needles: &["company.thebrowser", "arc.app", "\\arc\\", "arc.exe"],
        win: None, mac: Some("Arc/User Data"), linux: None },
    Known { family: "chromium", name: "Chromium", engine: Engine::Chromium, needles: &["chromium"],
        win: Some("Chromium/User Data"), mac: Some("Chromium"), linux: Some("chromium") },
    Known { family: "chrome", name: "Google Chrome", engine: Engine::Chromium, needles: &["chrome"],
        win: Some("Google/Chrome/User Data"), mac: Some("Google/Chrome"), linux: Some("google-chrome") },
    Known { family: "librewolf", name: "LibreWolf", engine: Engine::Firefox, needles: &["librewolf"],
        win: Some("librewolf"), mac: Some("librewolf"), linux: Some(".librewolf") },
    Known { family: "zen", name: "Zen", engine: Engine::Firefox, needles: &["zen"],
        win: Some("zen"), mac: Some("zen"), linux: Some(".zen") },
    Known { family: "tor", name: "Tor Browser", engine: Engine::Firefox, needles: &["tor browser", "torbrowser", "torproject"],
        win: None, mac: None, linux: None },
    Known { family: "waterfox", name: "Waterfox", engine: Engine::Firefox, needles: &["waterfox"],
        win: Some("Waterfox"), mac: Some("Waterfox"), linux: Some(".waterfox") },
    Known { family: "firefox", name: "Firefox", engine: Engine::Firefox, needles: &["firefox", "org.mozilla"],
        win: Some("Mozilla/Firefox"), mac: Some("Firefox"), linux: Some(".mozilla/firefox") },
    Known { family: "safari", name: "Safari", engine: Engine::Safari, needles: &["com.apple.safari", "safari.app"],
        win: None, mac: None, linux: None },
];

fn known_for(haystack: &str) -> Option<&'static Known> {
    let h = haystack.to_lowercase();
    KNOWN.iter().find(|k| k.needles.iter().any(|n| h.contains(n)))
}

/// Build a `Browser` from raw detection data, attaching family/engine/profiles when recognised.
pub fn make(name: &str, exec: String, exec_args: Vec<String>, hint: &str) -> Browser {
    let k = known_for(&format!("{name} {hint} {exec}"));
    let family = k.map(|k| k.family.to_string()).unwrap_or_else(|| slug(name));
    let engine = k.map(|k| k.engine).unwrap_or(Engine::Other);
    let profiles = k.map(profiles_for).unwrap_or_default();
    let trimmed = name.trim();
    let name = if trimmed.is_empty() { k.map(|k| k.name).unwrap_or("Browser") } else { trimmed };
    Browser {
        id: family.clone(),
        name: name.to_string(),
        family,
        engine,
        exec,
        exec_args,
        profiles,
        enabled: true,
        icon: None,
    }
}

/// Make ids unique when e.g. Chrome and Chrome Beta are both installed.
pub fn dedupe_ids(list: &mut [Browser]) {
    let mut seen = std::collections::HashMap::<String, usize>::new();
    for b in list.iter_mut() {
        let n = seen.entry(b.id.clone()).or_insert(0);
        *n += 1;
        if *n > 1 {
            b.id = format!("{}-{}", b.id, slug(&b.name));
        }
    }
}

pub fn slug(s: &str) -> String {
    let s: String = s
        .to_lowercase()
        .chars()
        .map(|c| if c.is_ascii_alphanumeric() { c } else { '-' })
        .collect();
    s.split('-').filter(|p| !p.is_empty()).collect::<Vec<_>>().join("-")
}

fn base_dir(k: &Known) -> Option<PathBuf> {
    let home = dirs::home_dir()?;
    if cfg!(windows) {
        let rel = k.win?;
        let root = if k.engine == Engine::Firefox { dirs::config_dir()? } else { dirs::data_local_dir()? };
        Some(root.join(rel))
    } else if cfg!(target_os = "macos") {
        Some(home.join("Library/Application Support").join(k.mac?))
    } else {
        let rel = k.linux?;
        if rel.starts_with('.') { Some(home.join(rel)) } else { Some(dirs::config_dir()?.join(rel)) }
    }
}

fn profiles_for(k: &Known) -> Vec<Profile> {
    let Some(dir) = base_dir(k) else { return vec![] };
    let list = match k.engine {
        Engine::Chromium => chromium_profiles(&dir),
        Engine::Firefox => firefox_profiles(&dir),
        _ => vec![],
    };
    // A single profile isn't worth showing.
    if list.len() > 1 { list } else { vec![] }
}

/// Parse Chromium `Local State` → `profile.info_cache`.
pub fn chromium_profiles(user_data: &Path) -> Vec<Profile> {
    let Ok(s) = std::fs::read_to_string(user_data.join("Local State")) else { return vec![] };
    parse_chromium_local_state(&s)
}

pub fn parse_chromium_local_state(s: &str) -> Vec<Profile> {
    let Ok(v) = serde_json::from_str::<serde_json::Value>(s) else { return vec![] };
    let Some(cache) = v.pointer("/profile/info_cache").and_then(|c| c.as_object()) else { return vec![] };
    let order: Vec<&str> = v
        .pointer("/profile/profiles_order")
        .and_then(|o| o.as_array())
        .map(|a| a.iter().filter_map(|x| x.as_str()).collect())
        .unwrap_or_default();

    let mut profiles: Vec<Profile> = cache
        .iter()
        .map(|(dir, info)| Profile { id: dir.clone(), name: chromium_profile_name(info).unwrap_or(dir).to_string() })
        .collect();

    // Follow Chromium's own order; unlisted profiles go last, with "Default" first among them.
    profiles.sort_by_key(|p| match order.iter().position(|o| *o == p.id) {
        Some(index) => index,
        None if p.id == "Default" => usize::MAX - 1,
        None => usize::MAX,
    });
    profiles
}

/// Chromium names unnamed profiles "Profile 1" / "Person 1"; prefer the account name then.
fn chromium_profile_name(info: &serde_json::Value) -> Option<&str> {
    let field = |key: &str| info.get(key).and_then(|n| n.as_str()).filter(|v| !v.is_empty());
    let name = field("name");
    let is_generic = name.is_none_or(|n| n.starts_with("Profile ") || n.starts_with("Person "));
    if is_generic {
        field("shortcut_name").or(field("gaia_name")).or(field("user_name")).or(name)
    } else {
        name
    }
}

/// Parse Firefox `profiles.ini`.
pub fn firefox_profiles(root: &Path) -> Vec<Profile> {
    let Ok(s) = std::fs::read_to_string(root.join("profiles.ini")) else { return vec![] };
    parse_firefox_ini(&s)
}

pub fn parse_firefox_ini(s: &str) -> Vec<Profile> {
    let mut out = vec![];
    let mut in_profile = false;
    for line in s.lines().map(str::trim) {
        if line.starts_with('[') {
            in_profile = line.to_lowercase().starts_with("[profile");
        } else if in_profile && let Some(name) = line.strip_prefix("Name=") {
            out.push(Profile { id: name.to_string(), name: name.to_string() });
        }
    }
    out
}

fn private_flag(b: &Browser) -> Option<&'static str> {
    match (b.engine, b.family.as_str()) {
        (Engine::Chromium, "edge") => Some("--inprivate"),
        (Engine::Chromium, "opera") => Some("--private"),
        (Engine::Chromium, _) => Some("--incognito"),
        (Engine::Firefox, _) => Some("--private-window"),
        _ => None,
    }
}

/// Arguments (excluding the executable) to open `url` in `b`.
pub fn launch_args(b: &Browser, url: &str, profile: Option<&str>, private: bool) -> Vec<String> {
    let mut a: Vec<String> = b.exec_args.clone();
    if let Some(p) = profile.filter(|p| !p.is_empty()) {
        match b.engine {
            Engine::Chromium => a.push(format!("--profile-directory={p}")),
            Engine::Firefox => {
                a.push("-P".into());
                a.push(p.into());
            }
            _ => {}
        }
    }
    if private {
        if let Some(f) = private_flag(b) {
            a.push(f.into());
        }
    } else if b.engine == Engine::Firefox && profile.is_some() {
        // Open in the (possibly already running) profile instance.
        a.push("--new-tab".into());
    }
    a.push(url.into());
    a
}

pub fn launch(b: &Browser, url: &str, profile: Option<&str>, private: bool) -> Result<(), String> {
    let args = launch_args(b, url, profile, private);
    if crate::config::is_e2e() {
        return log_e2e_launch(b, url, profile, private, &args);
    }
    let mut cmd = if cfg!(target_os = "macos") {
        let mut c = Command::new("open");
        // -n only when we must pass args (profiles/private); otherwise reuse the running instance.
        if args.len() > 1 {
            c.arg("-n").arg("-a").arg(&b.exec).arg("--args").args(&args);
        } else {
            c.arg("-a").arg(&b.exec).arg(url);
        }
        c
    } else {
        let mut c = Command::new(&b.exec);
        c.args(&args);
        c
    };
    #[cfg(windows)]
    {
        use std::os::windows::process::CommandExt;
        const DETACHED_PROCESS: u32 = 0x0000_0008;
        cmd.creation_flags(DETACHED_PROCESS);
    }
    cmd.spawn().map(|_| ()).map_err(|e| format!("Failed to launch {}: {e}", b.name))
}

/// E2E tests assert on `launches.jsonl` instead of starting real browsers.
fn log_e2e_launch(b: &Browser, url: &str, profile: Option<&str>, private: bool, args: &[String]) -> Result<(), String> {
    use std::io::Write;
    let line = serde_json::json!({ "browser": b.id, "url": url, "profile": profile, "private": private, "args": args });
    let mut file = std::fs::OpenOptions::new()
        .create(true)
        .append(true)
        .open(crate::config::dir().join("launches.jsonl"))
        .map_err(|e| e.to_string())?;
    writeln!(file, "{line}").map_err(|e| e.to_string())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_chromium_profiles_in_order() {
        let s = r#"{"profile":{"profiles_order":["Profile 1","Default"],"info_cache":{
            "Default":{"name":"Personal"},"Profile 1":{"name":"Work"}}}}"#;
        let p = parse_chromium_local_state(s);
        assert_eq!(p.iter().map(|p| p.name.as_str()).collect::<Vec<_>>(), ["Work", "Personal"]);
        assert_eq!(p[0].id, "Profile 1");
    }

    #[test]
    fn parses_firefox_ini() {
        let s = "[General]\nStartWithLastProfile=1\n\n[Profile1]\nName=work\nPath=x\n\n[Profile0]\nName=default-release\n\n[Install123]\nDefault=x\n";
        let p = parse_firefox_ini(s);
        assert_eq!(p.len(), 2);
        assert_eq!(p[0].name, "work");
    }

    #[test]
    fn recognises_families() {
        assert_eq!(make("Google Chrome", "C:/x/chrome.exe".into(), vec![], "").family, "chrome");
        assert_eq!(make("Microsoft Edge", "msedge.exe".into(), vec![], "").family, "edge");
        assert_eq!(make("Firefox Developer Edition", "firefox.exe".into(), vec![], "").engine, Engine::Firefox);
        assert_eq!(make("Weird Browser", "weird".into(), vec![], "").id, "weird-browser");
    }

    #[test]
    fn builds_args() {
        let mut b = make("Microsoft Edge", "msedge.exe".into(), vec![], "");
        b.profiles.clear();
        assert_eq!(
            launch_args(&b, "https://a.b", Some("Profile 1"), true),
            ["--profile-directory=Profile 1", "--inprivate", "https://a.b"]
        );
        let f = make("Firefox", "firefox".into(), vec![], "");
        assert_eq!(launch_args(&f, "https://a.b", None, true), ["--private-window", "https://a.b"]);
    }
}
