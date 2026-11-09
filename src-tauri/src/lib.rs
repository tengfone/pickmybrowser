mod browsers;
mod cleanup;
mod config;
mod icons;
mod platform;
mod urls;

use browsers::Browser;
use config::{Config, Recent, Rule, Settings};
use serde::Serialize;
use std::sync::Mutex;
use tauri::{
    AppHandle, Emitter, Manager, PhysicalPosition, PhysicalSize, RunEvent, WebviewUrl,
    WebviewWindow, WebviewWindowBuilder, WindowEvent,
    menu::{Menu, MenuItem, PredefinedMenuItem, Submenu},
    tray::{MouseButton, MouseButtonState, TrayIconBuilder, TrayIconEvent},
};
use tauri_plugin_autostart::ManagerExt;

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct RuleHit {
    pattern: String,
    browser_id: String,
    profile_id: Option<String>,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Pending {
    url: String,
    original: String,
    removed: usize,
    lookalike: Option<String>,
    rule: Option<RuleHit>,
    countdown_ms: u64,
}

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

struct AppState {
    cfg: Mutex<Config>,
    pending: Mutex<Option<Pending>>,
}

fn os() -> &'static str {
    if cfg!(windows) {
        "windows"
    } else if cfg!(target_os = "macos") {
        "macos"
    } else {
        "linux"
    }
}

/* ---------------------------------------------------------------- windows */

fn picker(app: &AppHandle) -> WebviewWindow {
    if let Some(w) = app.get_webview_window("picker") {
        return w;
    }
    let b = WebviewWindowBuilder::new(app, "picker", WebviewUrl::App("index.html".into()))
        .title("PickMyBrowser")
        .inner_size(460.0, 320.0)
        .decorations(false)
        .transparent(true)
        .shadow(false)
        .resizable(false)
        .always_on_top(true)
        .skip_taskbar(true)
        .visible(false)
        .focused(true);
    let w = b.build().expect("picker window");
    let h = w.clone();
    w.on_window_event(move |e| match e {
        WindowEvent::Focused(false) if !config::is_e2e() => {
            let _ = h.hide();
        }
        WindowEvent::CloseRequested { api, .. } => {
            api.prevent_close();
            let _ = h.hide();
        }
        _ => {}
    });
    w
}

fn show_settings(app: &AppHandle) {
    let w = match app.get_webview_window("settings") {
        Some(w) => w,
        None => {
            let w = WebviewWindowBuilder::new(app, "settings", WebviewUrl::App("index.html".into()))
                .title("PickMyBrowser")
                .inner_size(760.0, 540.0)
                .min_inner_size(640.0, 460.0)
                .center()
                .visible(false)
                .build()
                .expect("settings window");
            let h = w.clone();
            w.on_window_event(move |e| {
                if let WindowEvent::CloseRequested { api, .. } = e {
                    api.prevent_close();
                    let _ = h.hide();
                }
            });
            w
        }
    };
    let _ = w.show();
    let _ = w.unminimize();
    let _ = w.set_focus();
}

/* ------------------------------------------------------------ url intake */

fn normalise_arg(a: &str) -> Option<String> {
    let lower = a.to_lowercase();
    if lower.starts_with("http://") || lower.starts_with("https://") || lower.starts_with("file://") {
        return Some(a.to_string());
    }
    let p = std::path::Path::new(a);
    if p.is_file() && (lower.ends_with(".html") || lower.ends_with(".htm") || lower.ends_with(".xhtml")) {
        let abs = std::fs::canonicalize(p).ok()?;
        return url::Url::from_file_path(abs.to_string_lossy().trim_start_matches(r"\\?\")).ok().map(String::from);
    }
    None
}

fn handle_url(app: &AppHandle, raw: &str) {
    let st = app.state::<AppState>();
    let pending = {
        let mut cfg = st.cfg.lock().unwrap();
        let (url, removed) = if cfg.settings.strip_tracking { urls::strip_tracking(raw) } else { (raw.to_string(), 0) };
        let lookalike = if cfg.settings.warn_lookalike { urls::lookalike(&url) } else { None };
        let rule = urls::find_rule(&cfg.rules, &url)
            .filter(|r| cfg.browsers.iter().any(|b| b.id == r.browser_id))
            .map(|r| RuleHit { pattern: r.pattern.clone(), browser_id: r.browser_id.clone(), profile_id: r.profile_id.clone() });
        cfg.push_recent(&url, None);
        // A rule with no countdown opens straight away, unless the link looks suspicious.
        if let (Some(r), 0, None) = (&rule, cfg.settings.rule_countdown_ms, &lookalike) {
            if let Some(b) = cfg.browsers.iter().find(|b| b.id == r.browser_id).cloned() {
                let _ = browsers::launch(&b, &url, r.profile_id.as_deref(), false);
                cfg.save();
                return;
            }
        }
        cfg.save();
        Pending { url, original: raw.into(), removed, lookalike, rule, countdown_ms: cfg.settings.rule_countdown_ms }
    };
    *st.pending.lock().unwrap() = Some(pending.clone());
    let w = picker(app);
    let _ = w.emit_to("picker", "picker:show", pending);
}

fn handle_args(app: &AppHandle, args: &[String], first_run: bool) {
    if let Some(u) = args.iter().skip(1).find_map(|a| normalise_arg(a)) {
        handle_url(app, &u);
    } else if args.iter().any(|a| a == "--settings") || !(first_run && args.iter().any(|a| a == "--background")) {
        show_settings(app);
    }
}

/* -------------------------------------------------------------- commands */

#[tauri::command]
fn snapshot(app: AppHandle, st: tauri::State<AppState>) -> Snapshot {
    let c = st.cfg.lock().unwrap().clone();
    Snapshot {
        browsers: c.browsers,
        rules: c.rules,
        recents: c.recents,
        settings: c.settings,
        is_default: platform::is_default(),
        launch_at_login: app.autolaunch().is_enabled().unwrap_or(false),
        os: os(),
        version: env!("CARGO_PKG_VERSION"),
    }
}

#[tauri::command]
fn pending(st: tauri::State<AppState>) -> Option<Pending> {
    st.pending.lock().unwrap().clone()
}

/// Called by the picker once it has rendered and measured itself.
#[tauri::command]
fn present_picker(app: AppHandle, width: f64, height: f64) -> Result<(), String> {
    let w = picker(&app);
    let center = app.state::<AppState>().cfg.lock().unwrap().settings.position == "center";
    let cursor = app.cursor_position().map_err(|e| e.to_string())?;
    let mon = app
        .monitor_from_point(cursor.x, cursor.y)
        .ok()
        .flatten()
        .or_else(|| w.primary_monitor().ok().flatten());
    let scale = mon.as_ref().map(|m| m.scale_factor()).unwrap_or(1.0);
    let size = PhysicalSize::new((width * scale).round(), (height * scale).round());
    let _ = w.set_size(size);
    if let Some(m) = mon {
        let wa = m.work_area();
        let (ax, ay) = (wa.position.x as f64, wa.position.y as f64);
        let (aw, ah) = (wa.size.width as f64, wa.size.height as f64);
        let (mut x, mut y) = if center {
            (ax + (aw - size.width) / 2.0, ay + (ah - size.height) / 3.0)
        } else {
            (cursor.x - 24.0 * scale, cursor.y - 20.0 * scale)
        };
        let pad = 8.0 * scale;
        x = x.clamp(ax + pad, (ax + aw - size.width - pad).max(ax + pad));
        y = y.clamp(ay + pad, (ay + ah - size.height - pad).max(ay + pad));
        let _ = w.set_position(PhysicalPosition::new(x.round() as i32, y.round() as i32));
    }
    w.show().map_err(|e| e.to_string())?;
    let _ = w.set_focus();
    Ok(())
}

#[tauri::command]
fn dismiss(app: AppHandle) {
    if let Some(w) = app.get_webview_window("picker") {
        let _ = w.hide();
    }
}

#[tauri::command]
fn open_url(
    app: AppHandle,
    st: tauri::State<AppState>,
    url: String,
    browser_id: String,
    profile_id: Option<String>,
    private: bool,
) -> Result<(), String> {
    let mut cfg = st.cfg.lock().unwrap();
    let b = cfg.browsers.iter().find(|b| b.id == browser_id).cloned().ok_or("Unknown browser")?;
    browsers::launch(&b, &url, profile_id.as_deref(), private)?;
    if let Some(r) = cfg.recents.iter_mut().find(|r| r.url == url) {
        r.browser_id = Some(browser_id);
    }
    cfg.save();
    drop(cfg);
    *st.pending.lock().unwrap() = None;
    dismiss(app);
    Ok(())
}

/// Re-open the picker for a URL (from settings → recents or the tray).
#[tauri::command]
fn pick_url(app: AppHandle, url: String) {
    handle_url(&app, &url);
}

#[tauri::command]
async fn browser_icons(st: tauri::State<'_, AppState>) -> Result<std::collections::HashMap<String, String>, ()> {
    let list = st.cfg.lock().unwrap().browsers.clone();
    Ok(tauri::async_runtime::spawn_blocking(move || icons::all(&list)).await.unwrap_or_default())
}

#[tauri::command]
fn rescan(st: tauri::State<AppState>) -> Vec<Browser> {
    icons::clear_cache();
    let found = platform::detect();
    let mut c = st.cfg.lock().unwrap();
    c.merge_detected(found);
    c.save();
    c.browsers.clone()
}

#[tauri::command]
fn save_browsers(app: AppHandle, st: tauri::State<AppState>, browsers: Vec<Browser>) {
    let mut c = st.cfg.lock().unwrap();
    c.browsers = browsers;
    c.save();
    drop(c);
    refresh_tray(&app);
}

#[tauri::command]
fn save_rules(st: tauri::State<AppState>, rules: Vec<Rule>) {
    let mut c = st.cfg.lock().unwrap();
    c.rules = rules;
    c.save();
}

#[tauri::command]
fn save_settings(app: AppHandle, st: tauri::State<AppState>, settings: Settings) {
    let mut c = st.cfg.lock().unwrap();
    if !settings.store_recents {
        c.recents.clear();
    }
    c.settings = settings;
    let limit = c.settings.recent_limit.max(1);
    c.recents.truncate(limit);
    let show = c.settings.show_tray;
    c.save();
    drop(c);
    if let Some(t) = app.tray_by_id("main") {
        let _ = t.set_visible(show);
    }
    refresh_tray(&app);
    let _ = app.emit("settings:changed", ());
}

#[tauri::command]
fn clear_recents(app: AppHandle, st: tauri::State<AppState>) {
    let mut c = st.cfg.lock().unwrap();
    c.recents.clear();
    c.save();
    drop(c);
    refresh_tray(&app);
}

#[tauri::command]
fn set_default() -> Result<bool, String> {
    platform::set_default()?;
    Ok(platform::is_default())
}

#[tauri::command]
fn is_default() -> bool {
    platform::is_default()
}

#[tauri::command]
fn set_launch_at_login(app: AppHandle, on: bool) -> Result<(), String> {
    if config::is_e2e() {
        return Err("disabled in E2E".into());
    }
    let a = app.autolaunch();
    if on { a.enable() } else { a.disable() }.map_err(|e| e.to_string())
}

#[tauri::command]
fn reset_all(st: tauri::State<AppState>) -> Vec<Browser> {
    let mut c = st.cfg.lock().unwrap();
    *c = Config::default();
    c.merge_detected(platform::detect());
    c.save();
    c.browsers.clone()
}

#[tauri::command]
fn open_settings(app: AppHandle) {
    dismiss(app.clone());
    show_settings(&app);
}

/// Settings → "Remove PickMyBrowser": wipe registrations, autostart and data, then quit.
/// The app files themselves are removed by the OS uninstaller / Bin.
#[tauri::command]
fn uninstall_app(app: AppHandle) -> Result<(), String> {
    if config::is_e2e() {
        return Err("disabled in E2E".into());
    }
    cleanup::spawn_detached_uninstall()?;
    app.exit(0);
    Ok(())
}

#[tauri::command]
fn open_external(url: String) -> Result<(), String> {
    // Used for About links; route through the OS default (which may be us — that's fine).
    let (cmd, args): (&str, Vec<&str>) = if cfg!(windows) {
        ("cmd", vec!["/C", "start", "", &url])
    } else if cfg!(target_os = "macos") {
        ("open", vec![&url])
    } else {
        ("xdg-open", vec![&url])
    };
    std::process::Command::new(cmd).args(args).spawn().map(|_| ()).map_err(|e| e.to_string())
}

/* ------------------------------------------------------------------ tray */

fn build_menu(app: &AppHandle) -> tauri::Result<Menu<tauri::Wry>> {
    let recents = app.state::<AppState>().cfg.lock().unwrap().recents.clone();
    let sub = Submenu::with_id(app, "recents", "Recent links", !recents.is_empty())?;
    for (i, r) in recents.iter().take(10).enumerate() {
        let mut label = r.url.trim_start_matches("https://").trim_start_matches("http://").to_string();
        if label.chars().count() > 56 {
            label = label.chars().take(55).collect::<String>() + "…";
        }
        sub.append(&MenuItem::with_id(app, format!("recent:{i}"), label, true, None::<&str>)?)?;
    }
    let reopen = MenuItem::with_id(app, "reopen", "Reopen last link…", !recents.is_empty(), None::<&str>)?;
    let settings = MenuItem::with_id(app, "settings", "Settings…", true, None::<&str>)?;
    let quit = MenuItem::with_id(app, "quit", "Quit PickMyBrowser", true, None::<&str>)?;
    let sep = || PredefinedMenuItem::separator(app);
    Menu::with_items(app, &[&reopen, &sub, &sep()?, &settings, &sep()?, &quit])
}

fn refresh_tray(app: &AppHandle) {
    if let (Some(t), Ok(m)) = (app.tray_by_id("main"), build_menu(app)) {
        let _ = t.set_menu(Some(m));
    }
}

fn setup_tray(app: &AppHandle) -> tauri::Result<()> {
    let menu = build_menu(app)?;
    let tray = TrayIconBuilder::with_id("main")
        .icon(tauri::include_image!("icons/tray.png"))
        .tooltip("PickMyBrowser")
        .menu(&menu)
        .show_menu_on_left_click(cfg!(target_os = "macos"))
        .on_menu_event(|app, e| {
            let id = e.id().as_ref();
            let recent = |i: usize| app.state::<AppState>().cfg.lock().unwrap().recents.get(i).map(|r| r.url.clone());
            match id {
                "quit" => app.exit(0),
                "settings" => show_settings(app),
                "reopen" => {
                    if let Some(u) = recent(0) {
                        handle_url(app, &u)
                    }
                }
                _ => {
                    if let Some(i) = id.strip_prefix("recent:").and_then(|i| i.parse().ok()) {
                        if let Some(u) = recent(i) {
                            handle_url(app, &u)
                        }
                    }
                }
            }
        })
        .on_tray_icon_event(|t, e| {
            if let TrayIconEvent::Click { button: MouseButton::Left, button_state: MouseButtonState::Up, .. } = e {
                show_settings(t.app_handle());
            }
        })
        .build(app)?;
    let show = app.state::<AppState>().cfg.lock().unwrap().settings.show_tray;
    tray.set_visible(show)?;
    Ok(())
}

/* ------------------------------------------------------------------- run */

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    let args: Vec<String> = std::env::args().collect();
    if args.iter().any(|a| a == "--uninstall") {
        let left = cleanup::uninstall_all();
        if left.is_empty() {
            println!("PickMyBrowser registrations and settings removed.");
        } else {
            eprintln!("Could not remove: {left:?}");
            std::process::exit(1);
        }
        return;
    }

    let mut cfg = Config::load();
    if cfg.browsers.is_empty() {
        cfg.merge_detected(platform::detect());
        cfg.save();
    }

    let e2e = config::is_e2e();
    let mut builder = tauri::Builder::default();
    // E2E: several test runs may overlap a real install, so no single-instance lock.
    if !e2e {
        builder = builder.plugin(tauri_plugin_single_instance::init(|app, argv, _cwd| {
            handle_args(app, &argv, false);
        }));
    }
    // E2E-only: WebdriverIO bridge + embedded W3C WebDriver server (never in release builds).
    #[cfg(feature = "wdio")]
    {
        builder = builder.plugin(tauri_plugin_wdio::init()).plugin(tauri_plugin_wdio_webdriver::init());
    }
    let app = builder
        .plugin(
            tauri_plugin_autostart::Builder::new()
                .app_name("PickMyBrowser")
                .args(["--background"])
                .build(),
        )
        .manage(AppState { cfg: Mutex::new(cfg), pending: Mutex::new(None) })
        .invoke_handler(tauri::generate_handler![
            snapshot, pending, present_picker, dismiss, open_url, pick_url, rescan, save_browsers,
            save_rules, save_settings, clear_recents, set_default, is_default, set_launch_at_login,
            reset_all, open_settings, open_external, browser_icons, uninstall_app
        ])
        .setup(move |app| {
            #[cfg(target_os = "macos")]
            app.set_activation_policy(tauri::ActivationPolicy::Accessory);
            let h = app.handle().clone();
            if !e2e {
                setup_tray(&h)?;
            }
            picker(&h); // pre-create so it appears instantly
            #[cfg(not(target_os = "macos"))]
            handle_args(&h, &args, true);
            #[cfg(target_os = "macos")]
            if args.iter().any(|a| a == "--settings") || !args.iter().any(|a| a == "--background") {
                // On macOS, URLs arrive via RunEvent::Opened; give it a beat before opening settings.
                let h2 = h.clone();
                std::thread::spawn(move || {
                    std::thread::sleep(std::time::Duration::from_millis(400));
                    let has = h2.state::<AppState>().pending.lock().unwrap().is_some();
                    if !has {
                        let h3 = h2.clone();
                        let _ = h2.run_on_main_thread(move || show_settings(&h3));
                    }
                });
            }
            Ok(())
        })
        .build(tauri::generate_context!())
        .expect("error while building PickMyBrowser");

    app.run(|_app, event| match event {
        #[cfg(target_os = "macos")]
        RunEvent::Opened { urls } => {
            for u in urls {
                handle_url(_app, u.as_str());
            }
        }
        RunEvent::ExitRequested { api, code, .. } => {
            // Keep running in the tray when windows close; allow explicit quit (code set).
            if code.is_none() {
                api.prevent_exit();
            }
        }
        _ => {}
    });
}
