//! OS integration: browser detection and default-browser registration.
//!
//! Each OS module provides `detect`, `is_default`, `set_default` and `unregister`;
//! this module picks the right one and adds the cross-platform filtering.

use crate::browsers::Browser;
use crate::config;

#[cfg(windows)]
mod windows;
#[cfg(windows)]
use windows as os;

#[cfg(target_os = "macos")]
mod macos;
#[cfg(target_os = "macos")]
use macos as os;

#[cfg(all(unix, not(target_os = "macos")))]
mod linux;
#[cfg(all(unix, not(target_os = "macos")))]
use linux as os;
#[cfg(all(unix, not(target_os = "macos")))]
pub use linux::data_dirs as linux_data_dirs;

pub fn detect() -> Vec<Browser> {
    if config::is_e2e()
        && let Some(fixture) = detect_e2e_fixture() {
            return fixture;
        }

    let mut browsers = os::detect();
    browsers.retain(|browser| {
        let name = browser.name.to_lowercase();
        // Never offer ourselves, and IE can no longer open links on modern Windows.
        !name.contains("pickmybrowser") && !name.contains("internet explorer")
    });
    crate::browsers::dedupe_ids(&mut browsers);
    browsers
}

/// E2E runs use a fixed browser list written by the test runner, so results don't
/// depend on the machine. Returns `None` if the fixture is missing or invalid.
fn detect_e2e_fixture() -> Option<Vec<Browser>> {
    let json = std::fs::read_to_string(config::dir().join("fixture-browsers.json")).ok()?;
    let fixture: Vec<Browser> = serde_json::from_str(&json).ok()?;

    // Borrow the real install (exe / bundle / icon) when present, so the picker shows
    // real logos. Launches are only logged in E2E, so nothing is actually started.
    let installed = os::detect();
    let with_real_installs = fixture
        .into_iter()
        .map(|mut browser| {
            if let Some(real) = installed.iter().find(|real| real.family == browser.family) {
                browser.exec = real.exec.clone();
                browser.icon = real.icon.clone();
            }
            browser
        })
        .collect();
    Some(with_real_installs)
}

pub fn is_default() -> bool {
    os::is_default()
}

/// Register as an http/https handler and ask the OS to make us default
/// (opens system UI where the OS requires user consent).
pub fn set_default() -> Result<(), String> {
    if config::is_e2e() {
        return Err("disabled in E2E".into());
    }
    os::set_default()
}

/// Remove registrations made by `set_default` (used by `--uninstall`).
pub fn unregister() -> Result<(), String> {
    os::unregister()
}
