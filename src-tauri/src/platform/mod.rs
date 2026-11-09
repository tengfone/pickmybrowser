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
    imp::set_default()
}

/// Remove registrations made by `set_default` (used by `--uninstall`).
pub fn unregister() -> Result<(), String> {
    imp::unregister()
}
