//! Auto-update through the Tauri updater: a daily background check (if enabled) plus a
//! manual "Check for updates". Nothing is installed without the user saying so.
//!
//! Updates are signed with our own updater key (`plugins.updater.pubkey`), independent of
//! OS code signing. Linux deb/rpm installs are owned by the package manager, so those get
//! a link to the release page instead of an in-place install.

use crate::{AppState, config};
use serde::Serialize;
use std::sync::Mutex;
use std::time::{Duration, SystemTime, UNIX_EPOCH};
use tauri::{AppHandle, Emitter, Manager};
use tauri_plugin_updater::{Update, UpdaterExt};

pub const RELEASES_URL: &str = "https://github.com/tengfone/pickmybrowser/releases/latest";
const CHECK_EVERY_SECS: u64 = 24 * 60 * 60;
/// How often the background thread wakes up to see whether a check is due.
const POLL_EVERY: Duration = Duration::from_secs(60 * 60);
/// Let the app settle (and the network come up after login) before the first check.
const FIRST_POLL_DELAY: Duration = Duration::from_secs(30);

#[derive(Default)]
pub struct UpdateState(Mutex<Option<Update>>);

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct UpdateInfo {
    pub version: String,
    notes: Option<String>,
    /// True when this install can't update itself (Linux deb/rpm): show a download link.
    manual: bool,
    url: &'static str,
}

/// Linux packages from deb/rpm can't be replaced by the app; only the AppImage can.
pub fn can_self_update() -> bool {
    !cfg!(target_os = "linux") || std::env::var_os("APPIMAGE").is_some()
}

pub fn enabled() -> bool {
    !config::is_e2e()
}

fn now() -> u64 {
    SystemTime::now().duration_since(UNIX_EPOCH).map(|d| d.as_secs()).unwrap_or(0)
}

fn info(update: &Update) -> UpdateInfo {
    UpdateInfo {
        version: update.version.clone(),
        notes: update.body.clone(),
        manual: !can_self_update(),
        url: RELEASES_URL,
    }
}

/// The update found by the last check, if any.
pub fn available(app: &AppHandle) -> Option<UpdateInfo> {
    app.try_state::<UpdateState>()?.0.lock().unwrap().as_ref().map(info)
}

/// Asks the update endpoint for a newer version and remembers the result.
pub async fn check(app: &AppHandle) -> Result<Option<UpdateInfo>, String> {
    if !enabled() {
        return Ok(None);
    }
    let found = app.updater().map_err(|e| e.to_string())?.check().await.map_err(|e| e.to_string())?;
    {
        let state = app.state::<AppState>();
        let mut cfg = state.cfg.lock().unwrap();
        cfg.settings.last_update_check = now();
        cfg.save();
    }
    let result = found.as_ref().map(info);
    *app.state::<UpdateState>().0.lock().unwrap() = found;
    crate::tray::refresh(app);
    let _ = app.emit("update:changed", &result);
    Ok(result)
}

/// Downloads, verifies and installs the pending update, then restarts.
/// On Windows the NSIS installer closes the app itself.
pub async fn install(app: &AppHandle) -> Result<(), String> {
    if !can_self_update() {
        return Err("this install is managed by your package manager".into());
    }
    let update = app.state::<UpdateState>().0.lock().unwrap().clone();
    let Some(update) = update else { return Err("no update available".into()) };
    update.download_and_install(|_, _| {}, || {}).await.map_err(|e| e.to_string())?;
    app.restart();
}

/// Background loop: checks at most once a day while auto-update is on.
pub fn start_background_checks(app: &AppHandle) {
    let app = app.clone();
    tauri::async_runtime::spawn(async move {
        tokio_sleep(FIRST_POLL_DELAY).await;
        loop {
            let due = {
                let state = app.state::<AppState>();
                let settings = &state.cfg.lock().unwrap().settings;
                settings.auto_update && now().saturating_sub(settings.last_update_check) >= CHECK_EVERY_SECS
            };
            if due && let Err(e) = check(&app).await {
                eprintln!("update check failed: {e}");
            }
            tokio_sleep(POLL_EVERY).await;
        }
    });
}

async fn tokio_sleep(duration: Duration) {
    // Sleep on a blocking thread so we don't need tokio's time feature.
    let _ = tauri::async_runtime::spawn_blocking(move || std::thread::sleep(duration)).await;
}
