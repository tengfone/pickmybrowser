use crate::browsers::{self, Browser};
use std::path::{Path, PathBuf};
use std::process::Command;

const DESKTOP_ID: &str = "pickmybrowser.desktop";

fn data_dirs() -> Vec<PathBuf> {
    let mut v = vec![];
    if let Some(d) = dirs::data_dir() {
        v.push(d);
    }
    let sys = std::env::var("XDG_DATA_DIRS").unwrap_or_else(|_| "/usr/local/share:/usr/share".into());
    v.extend(sys.split(':').filter(|s| !s.is_empty()).map(PathBuf::from));
    v.push("/var/lib/flatpak/exports/share".into());
    if let Some(h) = dirs::home_dir() {
        v.push(h.join(".local/share/flatpak/exports/share"));
    }
    v
}

/// Minimal parse of the `[Desktop Entry]` group.
fn parse_desktop(s: &str) -> Option<(String, String, bool)> {
    let mut in_entry = false;
    let (mut name, mut exec, mut mime, mut hidden) = (None, None, String::new(), false);
    for line in s.lines().map(str::trim) {
        if line.starts_with('[') {
            in_entry = line == "[Desktop Entry]";
            continue;
        }
        if !in_entry {
            continue;
        }
        if let Some(v) = line.strip_prefix("Name=") {
            name.get_or_insert(v.to_string());
        } else if let Some(v) = line.strip_prefix("Exec=") {
            exec.get_or_insert(v.to_string());
        } else if let Some(v) = line.strip_prefix("MimeType=") {
            mime = v.to_string();
        } else if line == "NoDisplay=true" || line == "Hidden=true" {
            hidden = true;
        }
    }
    let handles = mime.split(';').any(|m| m == "x-scheme-handler/http" || m == "x-scheme-handler/https");
    Some((name?, exec?, handles && !hidden))
}

/// Split an Exec line, dropping field codes like %u %U %f.
pub fn split_exec(exec: &str) -> Vec<String> {
    let mut out = vec![];
    let mut cur = String::new();
    let mut quoted = false;
    for c in exec.chars() {
        match c {
            '"' => quoted = !quoted,
            ' ' if !quoted => {
                if !cur.is_empty() {
                    out.push(std::mem::take(&mut cur));
                }
            }
            _ => cur.push(c),
        }
    }
    if !cur.is_empty() {
        out.push(cur);
    }
    out.retain(|a| !(a.len() == 2 && a.starts_with('%')));
    out
}

pub fn detect() -> Vec<Browser> {
    let mut seen = std::collections::HashSet::new();
    let mut out = vec![];
    for d in data_dirs() {
        let Ok(rd) = std::fs::read_dir(d.join("applications")) else { continue };
        for e in rd.flatten() {
            let p = e.path();
            let id = p.file_name().unwrap_or_default().to_string_lossy().to_string();
            if !id.ends_with(".desktop") || id == DESKTOP_ID || !seen.insert(id.clone()) {
                continue;
            }
            let Ok(s) = std::fs::read_to_string(&p) else { continue };
            let Some((name, exec, ok)) = parse_desktop(&s) else { continue };
            if !ok {
                continue;
            }
            let mut parts = split_exec(&exec);
            if parts.is_empty() {
                continue;
            }
            let bin = parts.remove(0);
            out.push(browsers::make(&name, bin, parts, &id));
        }
    }
    out
}

pub fn is_default() -> bool {
    Command::new("xdg-settings")
        .args(["get", "default-web-browser"])
        .output()
        .map(|o| String::from_utf8_lossy(&o.stdout).trim() == DESKTOP_ID)
        .unwrap_or(false)
}

fn desktop_path() -> Option<PathBuf> {
    Some(dirs::data_dir()?.join("applications").join(DESKTOP_ID))
}

fn self_exec() -> String {
    // AppImage sets $APPIMAGE to the real, stable path.
    std::env::var("APPIMAGE")
        .ok()
        .or_else(|| std::env::current_exe().ok().map(|p| p.to_string_lossy().into()))
        .unwrap_or_else(|| "pickmybrowser".into())
}

pub fn set_default() -> Result<(), String> {
    let path = desktop_path().ok_or("no data dir")?;
    let entry = format!(
        "[Desktop Entry]\nType=Application\nName=PickMyBrowser\nComment=Pick which browser opens each link\nExec=\"{}\" %u\nIcon=pickmybrowser\nTerminal=false\nCategories=Network;WebBrowser;\nMimeType=x-scheme-handler/http;x-scheme-handler/https;text/html;\nStartupNotify=false\n",
        self_exec()
    );
    std::fs::create_dir_all(path.parent().unwrap_or(Path::new("."))).map_err(|e| e.to_string())?;
    std::fs::write(&path, entry).map_err(|e| e.to_string())?;
    let _ = Command::new("update-desktop-database").arg(path.parent().unwrap()).status();
    let ok = Command::new("xdg-settings")
        .args(["set", "default-web-browser", DESKTOP_ID])
        .status()
        .map(|s| s.success())
        .unwrap_or(false);
    if !ok {
        for m in ["x-scheme-handler/http", "x-scheme-handler/https"] {
            Command::new("xdg-mime").args(["default", DESKTOP_ID, m]).status().map_err(|e| e.to_string())?;
        }
    }
    Ok(())
}

pub fn unregister() -> Result<(), String> {
    if let Some(p) = desktop_path() {
        let _ = std::fs::remove_file(p);
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn parses_desktop_and_exec() {
        let s = "[Desktop Entry]\nName=Firefox\nName[de]=Feuerfuchs\nExec=/usr/lib/firefox/firefox %u\nMimeType=text/html;x-scheme-handler/http;\n[Desktop Action new]\nName=New\nExec=firefox --new-window\n";
        let (n, e, ok) = parse_desktop(s).unwrap();
        assert_eq!((n.as_str(), ok), ("Firefox", true));
        assert_eq!(split_exec(&e), ["/usr/lib/firefox/firefox"]);
        assert_eq!(split_exec("flatpak run \"com.brave.Browser\" @@u %U @@"), ["flatpak", "run", "com.brave.Browser", "@@u", "@@"]);
    }
}
