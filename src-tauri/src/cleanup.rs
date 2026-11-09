//! Full removal of everything PickMyBrowser writes outside its install location.
//! Run by `pickmybrowser --uninstall` (called by the Windows uninstaller and the Linux
//! package remove scripts) and by Settings → "Remove PickMyBrowser" (needed on macOS,
//! where dragging the app to the Bin runs no code).

use std::path::PathBuf;
use std::time::Duration;

pub const IDENTIFIER: &str = "com.tengfone.pickmybrowser";
/// Names the autostart entry may have been created under (current + early dev builds).
const AUTOSTART_NAMES: &[&str] = &["PickMyBrowser", "pickmybrowser-tauri", "pickmybrowser"];

/// Every per-user directory that may contain our data: settings + icon cache (config),
/// webview storage (data / local data), and caches.
pub fn data_dirs() -> Vec<PathBuf> {
    let mut v: Vec<PathBuf> = [dirs::config_dir(), dirs::data_dir(), dirs::data_local_dir(), dirs::cache_dir()]
        .into_iter()
        .flatten()
        .map(|d| d.join(IDENTIFIER))
        .collect();
    #[cfg(target_os = "macos")]
    if let Some(home) = dirs::home_dir() {
        let lib = home.join("Library");
        v.push(lib.join("WebKit").join(IDENTIFIER));
        v.push(lib.join("HTTPStorages").join(IDENTIFIER));
        v.push(lib.join("Saved Application State").join(format!("{IDENTIFIER}.savedState")));
        v.push(lib.join("Preferences").join(format!("{IDENTIFIER}.plist")));
    }
    v.sort();
    v.dedup();
    v
}

fn remove_autostart() {
    #[cfg(windows)]
    {
        use winreg::{RegKey, enums::*};
        let hk = RegKey::predef(HKEY_CURRENT_USER);
        for key in [
            r"Software\Microsoft\Windows\CurrentVersion\Run",
            r"Software\Microsoft\Windows\CurrentVersion\Explorer\StartupApproved\Run",
        ] {
            if let Ok(k) = hk.open_subkey_with_flags(key, KEY_WRITE) {
                for n in AUTOSTART_NAMES {
                    let _ = k.delete_value(n);
                }
            }
        }
        // Explorer remembers every tray icon (Settings → Taskbar → Other system tray icons).
        if let (Ok(exe), Ok(k)) = (std::env::current_exe(), hk.open_subkey(r"Control Panel\NotifyIconSettings")) {
            let exe = exe.to_string_lossy().to_lowercase();
            let ours: Vec<String> = k
                .enum_keys()
                .filter_map(|n| n.ok())
                .filter(|n| {
                    k.open_subkey(n)
                        .and_then(|s| s.get_value::<String, _>("ExecutablePath"))
                        .is_ok_and(|p| p.to_lowercase() == exe)
                })
                .collect();
            if let Ok(w) = hk.open_subkey_with_flags(r"Control Panel\NotifyIconSettings", KEY_ALL_ACCESS) {
                for n in ours {
                    let _ = w.delete_subkey_all(n);
                }
            }
        }
    }
    #[cfg(target_os = "macos")]
    if let Some(home) = dirs::home_dir() {
        for n in AUTOSTART_NAMES {
            let _ = std::fs::remove_file(home.join("Library/LaunchAgents").join(format!("{n}.plist")));
        }
    }
    #[cfg(all(unix, not(target_os = "macos")))]
    if let Some(cfg) = dirs::config_dir() {
        for n in AUTOSTART_NAMES {
            let _ = std::fs::remove_file(cfg.join("autostart").join(format!("{n}.desktop")));
        }
    }
}

fn remove_path(p: &PathBuf) -> bool {
    let r = if p.is_dir() { std::fs::remove_dir_all(p) } else { std::fs::remove_file(p) };
    r.is_ok() || !p.exists()
}

/// Remove registrations, autostart and all data. Retries for a few seconds because the
/// app (or its webview) may still be shutting down and holding files open.
/// Returns the paths that could not be removed.
pub fn uninstall_all() -> Vec<PathBuf> {
    let _ = crate::platform::unregister();
    remove_autostart();
    let dirs = data_dirs();
    for _ in 0..20 {
        if dirs.iter().all(remove_path) {
            return vec![];
        }
        std::thread::sleep(Duration::from_millis(500));
    }
    dirs.into_iter().filter(|p| p.exists()).collect()
}

/// Settings → "Remove PickMyBrowser": a detached copy of us does the cleanup once this
/// process (and its webview) have exited, so no files are locked.
pub fn spawn_detached_uninstall() -> Result<(), String> {
    let exe = std::env::current_exe().map_err(|e| e.to_string())?;
    let mut cmd = std::process::Command::new(exe);
    cmd.arg("--uninstall");
    #[cfg(windows)]
    {
        use std::os::windows::process::CommandExt;
        const DETACHED_PROCESS: u32 = 0x0000_0008;
        cmd.creation_flags(DETACHED_PROCESS);
    }
    cmd.spawn().map(|_| ()).map_err(|e| e.to_string())
}

#[cfg(test)]
mod tests {
    #[test]
    fn data_dirs_are_ours_only() {
        for d in super::data_dirs() {
            assert!(d.to_string_lossy().contains(super::IDENTIFIER), "{d:?}");
        }
    }
}
