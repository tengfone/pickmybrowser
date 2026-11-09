use crate::browsers::{self, Browser};
use std::collections::HashSet;
use std::path::{Path, PathBuf};
use std::process::Command;

const DESKTOP_ID: &str = "pickmybrowser.desktop";
const SCHEME_MIME_TYPES: [&str; 2] = ["x-scheme-handler/http", "x-scheme-handler/https"];

/// XDG data directories (user first), plus Flatpak's export dirs, which
/// aren't always listed in $XDG_DATA_DIRS.
pub fn data_dirs() -> Vec<PathBuf> {
    let mut dirs_found = vec![];
    if let Some(user_data) = dirs::data_dir() {
        dirs_found.push(user_data);
    }
    let system = std::env::var("XDG_DATA_DIRS").unwrap_or_else(|_| "/usr/local/share:/usr/share".into());
    dirs_found.extend(system.split(':').filter(|s| !s.is_empty()).map(PathBuf::from));
    dirs_found.push("/var/lib/flatpak/exports/share".into());
    if let Some(home) = dirs::home_dir() {
        dirs_found.push(home.join(".local/share/flatpak/exports/share"));
    }
    dirs_found
}

/// The fields we need from a `.desktop` file's `[Desktop Entry]` group.
struct DesktopEntry {
    name: String,
    exec: String,
    /// Visible and declares itself an http/https handler.
    is_browser: bool,
}

/// Minimal parse of the `[Desktop Entry]` group; other groups (actions) are ignored.
/// Only the first `Name=` counts, so localised `Name[xx]=` lines never win.
fn parse_desktop(contents: &str) -> Option<DesktopEntry> {
    let mut in_entry = false;
    let mut name = None;
    let mut exec = None;
    let mut mime_types = String::new();
    let mut hidden = false;
    for line in contents.lines().map(str::trim) {
        if line.starts_with('[') {
            in_entry = line == "[Desktop Entry]";
            continue;
        }
        if !in_entry {
            continue;
        }
        if let Some(value) = line.strip_prefix("Name=") {
            name.get_or_insert(value.to_string());
        } else if let Some(value) = line.strip_prefix("Exec=") {
            exec.get_or_insert(value.to_string());
        } else if let Some(value) = line.strip_prefix("MimeType=") {
            mime_types = value.to_string();
        } else if line == "NoDisplay=true" || line == "Hidden=true" {
            hidden = true;
        }
    }
    let handles_web = mime_types.split(';').any(|m| SCHEME_MIME_TYPES.contains(&m));
    Some(DesktopEntry {
        name: name?,
        exec: exec?,
        is_browser: handles_web && !hidden,
    })
}

/// Split an Exec line into arguments, dropping field codes like %u %U %f.
pub fn split_exec(exec: &str) -> Vec<String> {
    let mut args = vec![];
    let mut current = String::new();
    let mut quoted = false;
    for c in exec.chars() {
        match c {
            '"' => quoted = !quoted,
            ' ' if !quoted => {
                if !current.is_empty() {
                    args.push(std::mem::take(&mut current));
                }
            }
            _ => current.push(c),
        }
    }
    if !current.is_empty() {
        args.push(current);
    }
    args.retain(|arg| !(arg.len() == 2 && arg.starts_with('%')));
    args
}

pub fn detect() -> Vec<Browser> {
    // A desktop ID in an earlier (higher-priority) data dir shadows later ones.
    let mut seen_ids = HashSet::new();
    let mut found = vec![];
    for data_dir in data_dirs() {
        let Ok(entries) = std::fs::read_dir(data_dir.join("applications")) else { continue };
        for entry in entries.flatten() {
            let path = entry.path();
            let id = path.file_name().unwrap_or_default().to_string_lossy().to_string();
            if !id.ends_with(".desktop") || id == DESKTOP_ID || !seen_ids.insert(id.clone()) {
                continue;
            }
            if let Some(browser) = read_browser(&path, &id) {
                found.push(browser);
            }
        }
    }
    found
}

fn read_browser(path: &Path, id: &str) -> Option<Browser> {
    let contents = std::fs::read_to_string(path).ok()?;
    let entry = parse_desktop(&contents)?;
    if !entry.is_browser {
        return None;
    }
    let mut args = split_exec(&entry.exec);
    if args.is_empty() {
        return None;
    }
    let program = args.remove(0);
    let mut browser = browsers::make(&entry.name, program, args, id);
    browser.icon = first_group_icon(&contents);
    Some(browser)
}

/// The first `Icon=` line before the second group header (normally the `[Desktop Entry]` group).
fn first_group_icon(contents: &str) -> Option<String> {
    let first_group = contents.split("\n[").next()?;
    first_group
        .lines()
        .find_map(|line| line.trim().strip_prefix("Icon=").map(|value| value.trim().to_string()))
}

pub fn is_default() -> bool {
    Command::new("xdg-settings")
        .args(["get", "default-web-browser"])
        .output()
        .map(|o| String::from_utf8_lossy(&o.stdout).trim() == DESKTOP_ID)
        .unwrap_or(false)
}

fn our_desktop_file() -> Option<PathBuf> {
    Some(dirs::data_dir()?.join("applications").join(DESKTOP_ID))
}

fn self_exec() -> String {
    // AppImage sets $APPIMAGE to the real, stable path; current_exe points into a temp mount.
    std::env::var("APPIMAGE")
        .ok()
        .or_else(|| std::env::current_exe().ok().map(|p| p.to_string_lossy().into()))
        .unwrap_or_else(|| "pickmybrowser".into())
}

fn desktop_entry_contents() -> String {
    format!(
        "[Desktop Entry]\n\
         Type=Application\n\
         Name=PickMyBrowser\n\
         Comment=Pick which browser opens each link\n\
         Exec=\"{}\" %u\n\
         Icon=pickmybrowser\n\
         Terminal=false\n\
         Categories=Network;WebBrowser;\n\
         MimeType=x-scheme-handler/http;x-scheme-handler/https;text/html;\n\
         StartupNotify=false\n",
        self_exec()
    )
}

/// Refresh the MIME cache so the change is seen immediately; missing tool is fine.
fn update_desktop_database(applications_dir: &Path) {
    let _ = Command::new("update-desktop-database").arg(applications_dir).status();
}

pub fn set_default() -> Result<(), String> {
    let desktop_file = our_desktop_file().ok_or("no data dir")?;
    let applications_dir = desktop_file.parent().unwrap_or(Path::new("."));
    std::fs::create_dir_all(applications_dir).map_err(|e| e.to_string())?;
    std::fs::write(&desktop_file, desktop_entry_contents()).map_err(|e| e.to_string())?;
    update_desktop_database(applications_dir);

    // xdg-settings knows each desktop's own mechanism; fall back to plain
    // mimeapps.list associations via xdg-mime when it fails or is missing.
    let set_via_xdg_settings = Command::new("xdg-settings")
        .args(["set", "default-web-browser", DESKTOP_ID])
        .status()
        .map(|s| s.success())
        .unwrap_or(false);
    if !set_via_xdg_settings {
        for mime in SCHEME_MIME_TYPES {
            Command::new("xdg-mime")
                .args(["default", DESKTOP_ID, mime])
                .status()
                .map_err(|e| e.to_string())?;
        }
    }
    Ok(())
}

pub fn unregister() -> Result<(), String> {
    if let Some(desktop_file) = our_desktop_file() {
        let _ = std::fs::remove_file(&desktop_file);
        if let Some(applications_dir) = desktop_file.parent() {
            update_desktop_database(applications_dir);
        }
    }

    // xdg-mime / xdg-settings record us in mimeapps.list; strip those entries.
    let mut mimeapps_files = vec![];
    if let Some(config) = dirs::config_dir() {
        mimeapps_files.push(config.join("mimeapps.list"));
    }
    if let Some(data) = dirs::data_dir() {
        mimeapps_files.push(data.join("applications").join("mimeapps.list"));
    }
    for file in mimeapps_files {
        let Ok(contents) = std::fs::read_to_string(&file) else { continue };
        let cleaned = strip_mimeapps(&contents);
        if cleaned != contents {
            let _ = std::fs::write(&file, cleaned);
        }
    }
    Ok(())
}

/// Remove `pickmybrowser.desktop` from every association line; drop lines left empty.
fn strip_mimeapps(contents: &str) -> String {
    let mut out = String::new();
    for line in contents.lines() {
        let is_group_header = line.trim_start().starts_with('[');
        match line.split_once('=') {
            Some((mime, handlers)) if !is_group_header && handlers.contains(DESKTOP_ID) => {
                let others: Vec<&str> = handlers
                    .split(';')
                    .filter(|h| !h.is_empty() && *h != DESKTOP_ID)
                    .collect();
                if !others.is_empty() {
                    out.push_str(&format!("{mime}={};\n", others.join(";")));
                }
            }
            _ => {
                out.push_str(line);
                out.push('\n');
            }
        }
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_desktop_and_exec() {
        let s = "[Desktop Entry]\nName=Firefox\nName[de]=Feuerfuchs\nExec=/usr/lib/firefox/firefox %u\nMimeType=text/html;x-scheme-handler/http;\n[Desktop Action new]\nName=New\nExec=firefox --new-window\n";
        let entry = parse_desktop(s).unwrap();
        assert_eq!((entry.name.as_str(), entry.is_browser), ("Firefox", true));
        assert_eq!(split_exec(&entry.exec), ["/usr/lib/firefox/firefox"]);
        let m = "[Default Applications]\nx-scheme-handler/http=pickmybrowser.desktop\nx-scheme-handler/https=pickmybrowser.desktop;firefox.desktop;\n[Added Associations]\ntext/html=firefox.desktop;\n";
        assert_eq!(strip_mimeapps(m), "[Default Applications]\nx-scheme-handler/https=firefox.desktop;\n[Added Associations]\ntext/html=firefox.desktop;\n");
        assert_eq!(split_exec("flatpak run \"com.brave.Browser\" @@u %U @@"), ["flatpak", "run", "com.brave.Browser", "@@u", "@@"]);
    }
}
