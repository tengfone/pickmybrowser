//! OS integration: browser detection and default-browser registration.

use crate::browsers::Browser;

#[cfg(windows)]
mod windows;
#[cfg(windows)]
use windows as imp;

#[cfg(target_os = "macos")]
mod macos;
#[cfg(target_os = "macos")]
use macos as imp;

#[cfg(all(unix, not(target_os = "macos")))]
mod linux;
#[cfg(all(unix, not(target_os = "macos")))]
use linux as imp;
#[cfg(all(unix, not(target_os = "macos")))]
pub use linux::data_dirs as linux_data_dirs;

pub fn detect() -> Vec<Browser> {
    // E2E: a fixed browser list written by the test runner, so results don't depend on the machine.
    if crate::config::is_e2e() {
        if let Some(list) = std::fs::read_to_string(crate::config::dir().join("fixture-browsers.json"))
            .ok()
            .and_then(|s| serde_json::from_str::<Vec<Browser>>(&s).ok())
        {
            // Borrow the real install (exe / bundle / icon) when present, so the picker shows
            // real logos. Launches are only logged in E2E, so nothing is actually started.
            let real = imp::detect();
            return list
                .into_iter()
                .map(|mut b| {
                    if let Some(r) = real.iter().find(|r| r.family == b.family) {
                        b.exec = r.exec.clone();
                        b.icon = r.icon.clone();
                    }
                    b
                })
                .collect();
        }
    }
    let mut list = imp::detect();
    list.retain(|b| {
        let n = b.name.to_lowercase();
        !n.contains("pickmybrowser") && !n.contains("internet explorer")
    });
    crate::browsers::dedupe_ids(&mut list);
    list
}

pub fn is_default() -> bool {
    imp::is_default()
}

/// Register as an http/https handler and ask the OS to make us default
/// (opens system UI where the OS requires user consent).
pub fn set_default() -> Result<(), String> {
    if crate::config::is_e2e() {
        return Err("disabled in E2E".into());
    }
    imp::set_default()
}

/// Remove registrations made by `set_default` (used by `--uninstall`).
pub fn unregister() -> Result<(), String> {
    imp::unregister()
}
