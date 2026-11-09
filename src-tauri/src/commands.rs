//! Tauri commands invoked by the Svelte frontend. Argument names are part of the
//! frontend contract (sent as camelCase JSON), so don't rename them.

use crate::browsers::{self, Browser};
use crate::config::{self, Config, Recent, Rule, Settings};
use crate::{AppState, Pending, app_windows, cleanup, icons, open_in_picker, platform, tray};
use serde::Serialize;
use std::collections::HashMap;
use tauri::{AppHandle, Emitter, State};
use tauri_plugin_autostart::ManagerExt;

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Snapshot {
    browsers: Vec<Browser>,
    rules: Vec<Rule>,
    recents: Vec<Recent>,
    settings: Settings,
    is_default: bool,
    launch_at_login: bool,
    os: &'static str,
    version: &'static str,
}

fn os_name() -> &'static str {
    if cfg!(windows) {
        "windows"
    } else if cfg!(target_os = "macos") {
        "macos"
    } else {
        "linux"
    }
}

#[tauri::command]
pub fn snapshot(app: AppHandle, st: State<AppState>) -> Snapshot {
    let cfg = st.cfg.lock().unwrap().clone();
    Snapshot {
        browsers: cfg.browsers,
        rules: cfg.rules,
        recents: cfg.recents,
        settings: cfg.settings,
        is_default: platform::is_default(),
        launch_at_login: app.autolaunch().is_enabled().unwrap_or(false),
        os: os_name(),
        version: env!("CARGO_PKG_VERSION"),
    }
}

#[tauri::command]
pub fn pending(st: State<AppState>) -> Option<Pending> {
    crate::front_pending(&st)
}

/// Called by the picker once it has rendered and measured itself.
#[tauri::command]
pub fn present_picker(app: AppHandle, st: State<AppState>, width: f64, height: f64) -> Result<(), String> {
    let centered = st.cfg.lock().unwrap().settings.position == "center";
    app_windows::present_picker(&app, width, height, centered)
}

#[tauri::command]
pub fn resize_picker(app: AppHandle, width: f64, height: f64) -> Result<(), String> {
    app_windows::resize_picker(&app, width, height)
}

/// The front link was dismissed: drop it and show the next one, or hide the picker.
#[tauri::command]
pub fn dismiss(app: AppHandle) {
    if !crate::advance_pending(&app) {
        app_windows::hide_picker(&app);
    }
}

#[tauri::command]
pub fn open_url(
    app: AppHandle,
    st: State<AppState>,
    url: String,
    browser_id: String,
    profile_id: Option<String>,
    private: bool,
) -> Result<(), String> {
    {
        let mut cfg = st.cfg.lock().unwrap();
        let browser = cfg.browsers.iter().find(|b| b.id == browser_id).cloned().ok_or("Unknown browser")?;
        browsers::launch(&browser, &url, profile_id.as_deref(), private)?;
        if let Some(recent) = cfg.recents.iter_mut().find(|r| r.url == url) {
            recent.browser_id = Some(browser_id);
        }
        cfg.save();
    }
    if !crate::advance_pending(&app) {
        app_windows::hide_picker(&app);
    }
    Ok(())
}

/// Re-open the picker for a URL (from settings → recents).
#[tauri::command]
pub fn pick_url(app: AppHandle, url: String) {
    open_in_picker(&app, &url);
}

#[tauri::command]
pub async fn browser_icons(st: State<'_, AppState>) -> Result<HashMap<String, String>, ()> {
    let list = st.cfg.lock().unwrap().browsers.clone();
    // Icon extraction touches the disk and OS APIs; keep it off the async runtime.
    let icons = tauri::async_runtime::spawn_blocking(move || icons::all(&list)).await;
    Ok(icons.unwrap_or_default())
}

#[tauri::command]
pub fn rescan(st: State<AppState>) -> Vec<Browser> {
    icons::clear_cache();
    let detected = platform::detect();
    let mut cfg = st.cfg.lock().unwrap();
    cfg.merge_detected(detected);
    cfg.save();
    cfg.browsers.clone()
}

#[tauri::command]
pub fn save_browsers(app: AppHandle, st: State<AppState>, mut browsers: Vec<Browser>) {
    // Custom keys: one lowercase letter or digit each, and no two browsers sharing one.
    let mut taken = std::collections::HashSet::new();
    for browser in &mut browsers {
        browser.hotkey = browser
            .hotkey
            .take()
            .map(|k| k.to_ascii_lowercase())
            .filter(|k| k.len() == 1 && k.chars().all(|c| c.is_ascii_alphanumeric()))
            .filter(|k| taken.insert(k.clone()));
    }
    {
        let mut cfg = st.cfg.lock().unwrap();
        cfg.browsers = browsers;
        cfg.save();
    }
    tray::refresh(&app);
}

/// Which rule (index into `rules`) a URL would match, for the "Test a link" box. Uses the
/// same matcher as real links; tracking stripping doesn't change the host/path it checks.
#[tauri::command]
pub fn test_rule(rules: Vec<Rule>, url: String) -> Option<usize> {
    let url = if url.contains("://") { url } else { format!("https://{url}") };
    rules.iter().position(|r| r.enabled && crate::urls::rule_matches(&r.pattern, &url))
}

#[tauri::command]
pub fn save_rules(st: State<AppState>, rules: Vec<Rule>) {
    let mut cfg = st.cfg.lock().unwrap();
    cfg.rules = rules;
    cfg.save();
}

#[tauri::command]
pub fn save_settings(app: AppHandle, st: State<AppState>, settings: Settings) {
    let show_tray = {
        let mut cfg = st.cfg.lock().unwrap();
        if !settings.store_recents {
            cfg.recents.clear();
        }
        // The backend owns the check time; the frontend copy may be stale.
        let last_update_check = cfg.settings.last_update_check;
        cfg.settings = settings;
        cfg.settings.last_update_check = last_update_check;
        let limit = cfg.settings.recent_limit.max(1);
        cfg.recents.truncate(limit);
        cfg.save();
        cfg.settings.show_tray
    };
    tray::set_visible(&app, show_tray);
    tray::refresh(&app);
    let _ = app.emit("settings:changed", ());
}

#[tauri::command]
pub fn clear_recents(app: AppHandle, st: State<AppState>) {
    {
        let mut cfg = st.cfg.lock().unwrap();
        cfg.recents.clear();
        cfg.save();
    }
    tray::refresh(&app);
}

#[tauri::command]
pub fn set_default() -> Result<bool, String> {
    platform::set_default()?;
    Ok(platform::is_default())
}

#[tauri::command]
pub fn is_default() -> bool {
    platform::is_default()
}

#[tauri::command]
pub fn set_launch_at_login(app: AppHandle, on: bool) -> Result<(), String> {
    if config::is_e2e() {
        return Err("disabled in E2E".into());
    }
    let autolaunch = app.autolaunch();
    let result = if on { autolaunch.enable() } else { autolaunch.disable() };
    result.map_err(|e| e.to_string())
}

#[tauri::command]
pub fn reset_all(st: State<AppState>) -> Vec<Browser> {
    let mut cfg = st.cfg.lock().unwrap();
    *cfg = Config::default();
    cfg.merge_detected(platform::detect());
    cfg.save();
    cfg.browsers.clone()
}

#[tauri::command]
pub fn open_settings(app: AppHandle) {
    app_windows::hide_picker(&app);
    app_windows::show_settings(&app);
}

/// Settings → "Remove PickMyBrowser": wipe registrations, autostart and data, then quit.
/// The app files themselves are removed by the OS uninstaller / Bin.
#[tauri::command]
pub fn uninstall_app(app: AppHandle) -> Result<(), String> {
    if config::is_e2e() {
        return Err("disabled in E2E".into());
    }
    cleanup::spawn_detached_uninstall()?;
    app.exit(0);
    Ok(())
}

/// Used for About links. Goes through the OS default browser, which may be us; that's fine.
#[tauri::command]
pub fn open_external(url: String) -> Result<(), String> {
    // Only plain https links (the About page's links). On Windows the URL goes straight to
    // the URL handler (no cmd, so no console flash and no shell parsing of & or |).
    let parsed = url::Url::parse(&url).map_err(|e| e.to_string())?;
    let plain = url.chars().all(|c| c.is_ascii_alphanumeric() || "-._~:/?#=%+,@".contains(c));
    if parsed.scheme() != "https" || !plain {
        return Err("only plain https links can be opened".into());
    }
    let mut cmd = if cfg!(windows) {
        let mut c = std::process::Command::new("rundll32");
        c.arg("url.dll,FileProtocolHandler");
        #[cfg(windows)]
        {
            use std::os::windows::process::CommandExt;
            const CREATE_NO_WINDOW: u32 = 0x0800_0000;
            c.creation_flags(CREATE_NO_WINDOW);
        }
        c
    } else if cfg!(target_os = "macos") {
        std::process::Command::new("open")
    } else {
        std::process::Command::new("xdg-open")
    };
    cmd.arg(&url);
    cmd.spawn().map(|_| ()).map_err(|e| e.to_string())
}

/* ---------------------------------------------------------------- updates */

/// The update found by the last check (background or manual), if any.
#[tauri::command]
pub fn update_status(app: AppHandle) -> Option<crate::updates::UpdateInfo> {
    crate::updates::available(&app)
}

#[tauri::command]
pub async fn check_update(app: AppHandle) -> Result<Option<crate::updates::UpdateInfo>, String> {
    crate::updates::check(&app).await
}

#[tauri::command]
pub async fn install_update(app: AppHandle) -> Result<(), String> {
    crate::updates::install(&app).await
}
