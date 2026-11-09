mod app_windows;
mod browsers;
mod cleanup;
mod commands;
mod config;
mod icons;
mod platform;
mod tray;
mod updates;
mod urls;

use config::Config;
use serde::Serialize;
use std::sync::Mutex;
use tauri::{AppHandle, Emitter, Manager, RunEvent};

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct RuleHit {
    pattern: String,
    browser_id: String,
    profile_id: Option<String>,
}

/// The link currently waiting for the user to pick a browser.
#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Pending {
    url: String,
    original: String,
    removed: usize,
    lookalike: Option<String>,
    rule: Option<RuleHit>,
    countdown_ms: u64,
    /// Links still waiting behind this one (clicked before this one was picked).
    queued: usize,
    /// Set when a rule's browser couldn't be started, so the picker can say why it's showing.
    notice: Option<String>,
}

struct AppState {
    cfg: Mutex<Config>,
    /// Links waiting for a browser, oldest first. The picker shows the front one.
    pending: Mutex<std::collections::VecDeque<Pending>>,
}

/// The link the picker should show now, with the count of links waiting behind it.
pub(crate) fn front_pending(state: &AppState) -> Option<Pending> {
    let queue = state.pending.lock().unwrap();
    queue.front().cloned().map(|mut p| {
        p.queued = queue.len() - 1;
        p
    })
}

/// Forget every waiting link (the user clicked away from the picker).
pub(crate) fn clear_pending(app: &AppHandle) {
    app.state::<AppState>().pending.lock().unwrap().clear();
}

/// Drops the front link (opened or dismissed) and shows the next one, if any.
/// Returns true when another link is now showing.
pub(crate) fn advance_pending(app: &AppHandle) -> bool {
    let state = app.state::<AppState>();
    state.pending.lock().unwrap().pop_front();
    match front_pending(&state) {
        Some(next) => {
            let _ = app.emit_to(app_windows::PICKER, "picker:show", next);
            true
        }
        None => false,
    }
}

/* ------------------------------------------------------------ url intake */

/// Accepts http(s) URLs and local file:// URLs, and turns a local HTML file path into a
/// file:// URL. The URL is parsed and re-serialised, so the rule check, the picker and the
/// browser all see exactly the same link.
fn url_from_arg(arg: &str) -> Option<String> {
    if let Ok(url) = url::Url::parse(arg) {
        return match url.scheme() {
            "http" | "https" => Some(url.into()),
            // file://server/share would make the browser reach out over SMB; local files only.
            "file" if matches!(url.host_str(), None | Some("") | Some("localhost")) => Some(url.into()),
            _ => None,
        };
    }
    let lower = arg.to_lowercase();
    let path = std::path::Path::new(arg);
    let is_html = lower.ends_with(".html") || lower.ends_with(".htm") || lower.ends_with(".xhtml");
    if !(is_html && path.is_file()) {
        return None;
    }
    let absolute = std::fs::canonicalize(path).ok()?;
    // canonicalize() yields a verbatim `\\?\C:\...` path on Windows, which Url rejects.
    let absolute = absolute.to_string_lossy();
    let absolute = absolute.trim_start_matches(r"\\?\");
    url::Url::from_file_path(absolute).ok().map(String::from)
}

/// Cleans up an incoming link, records it, and either opens it straight away (rule with no
/// countdown) or hands it to the picker.
fn open_in_picker(app: &AppHandle, raw: &str) {
    let state = app.state::<AppState>();
    let pending = {
        let mut cfg = state.cfg.lock().unwrap();
        let (url, removed) = if cfg.settings.strip_tracking {
            urls::strip_tracking(raw)
        } else {
            (raw.to_string(), 0)
        };
        let lookalike = if cfg.settings.warn_lookalike { urls::lookalike(&url) } else { None };
        let rule = urls::find_rule(&cfg.rules, &url)
            .filter(|r| cfg.browsers.iter().any(|b| b.id == r.browser_id))
            .map(|r| RuleHit {
                pattern: r.pattern.clone(),
                browser_id: r.browser_id.clone(),
                profile_id: r.profile_id.clone(),
            });
        cfg.push_recent(&url, None);

        // A rule with no countdown opens straight away, unless the link looks suspicious.
        // If the browser won't start (uninstalled, moved), fall back to the picker instead
        // of losing the link.
        let countdown_ms = cfg.settings.rule_countdown_ms;
        let open_immediately = countdown_ms == 0 && lookalike.is_none();
        let mut notice = None;
        let mut rule = rule;
        if open_immediately
            && let Some(hit) = &rule
            && let Some(browser) = cfg.browsers.iter().find(|b| b.id == hit.browser_id).cloned()
        {
            match browsers::launch(&browser, &url, hit.profile_id.as_deref(), false) {
                Ok(()) => {
                    cfg.save();
                    return;
                }
                Err(e) => {
                    notice = Some(format!("{} couldn't be started ({e}). Pick another browser.", browser.name));
                    rule = None;
                }
            }
        }
        cfg.save();
        Pending { url, original: raw.into(), removed, lookalike, rule, countdown_ms, queued: 0, notice }
    };
    let next = {
        let mut queue = state.pending.lock().unwrap();
        // The same link clicked twice in a row only needs picking once.
        if queue.back().is_some_and(|p| p.url == pending.url) {
            None
        } else {
            queue.push_back(pending);
            Some(queue.len())
        }
    };
    app_windows::picker(app);
    match next {
        // First in line: show it now.
        Some(1) => {
            let _ = app.emit_to(app_windows::PICKER, "picker:show", front_pending(&state));
        }
        // Others wait their turn. If the picker was closed (it hides when it loses focus),
        // bring the waiting link back; otherwise just update its "N more" count.
        _ => {
            if app_windows::picker(app).is_visible().unwrap_or(false) {
                let _ = app.emit_to(app_windows::PICKER, "picker:queued", state.pending.lock().unwrap().len() - 1);
            } else {
                let _ = app.emit_to(app_windows::PICKER, "picker:show", front_pending(&state));
            }
        }
    }
}

/// Our own flags are only honoured as the sole argument (`pickmybrowser --background`), so a
/// link can never smuggle one in next to a URL.
fn has_flag(args: &[String], flag: &str) -> bool {
    args.len() == 2 && args[1] == flag
}

/// `first_run` is false when the args were forwarded from a second instance.
fn handle_args(app: &AppHandle, args: &[String], first_run: bool) {
    if let Some(url) = args.iter().skip(1).find_map(|a| url_from_arg(a)) {
        open_in_picker(app, &url);
        return;
    }
    // Autostart passes --background so login doesn't pop the settings window.
    let started_in_background = first_run && has_flag(args, "--background");
    if has_flag(args, "--settings") || !started_in_background {
        app_windows::show_settings(app);
    }
}

/// On macOS URLs arrive via `RunEvent::Opened` after startup, so wait briefly and only
/// show settings if no link turned up.
#[cfg(target_os = "macos")]
fn show_settings_unless_link_arrives(app: &AppHandle) {
    let app = app.clone();
    std::thread::spawn(move || {
        std::thread::sleep(std::time::Duration::from_millis(400));
        let has_pending = !app.state::<AppState>().pending.lock().unwrap().is_empty();
        if !has_pending {
            let handle = app.clone();
            let _ = app.run_on_main_thread(move || app_windows::show_settings(&handle));
        }
    });
}

/* ------------------------------------------------------------------- run */

fn run_uninstall() {
    let left = cleanup::uninstall_all();
    if left.is_empty() {
        println!("PickMyBrowser registrations and settings removed.");
    } else {
        eprintln!("Could not remove: {left:?}");
        std::process::exit(1);
    }
}

fn load_config() -> Config {
    let mut cfg = Config::load();
    if cfg.browsers.is_empty() {
        cfg.merge_detected(platform::detect());
        cfg.save();
    }
    cfg
}

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    let args: Vec<String> = std::env::args().collect();
    if has_flag(&args, "--uninstall") {
        run_uninstall();
        return;
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
        .plugin(tauri_plugin_updater::Builder::new().build())
        .manage(updates::UpdateState::default())
        .manage(AppState { cfg: Mutex::new(load_config()), pending: Mutex::new(Default::default()) })
        .invoke_handler(tauri::generate_handler![
            commands::snapshot,
            commands::pending,
            commands::present_picker,
            commands::resize_picker,
            commands::dismiss,
            commands::open_url,
            commands::pick_url,
            commands::rescan,
            commands::save_browsers,
            commands::save_rules,
            commands::test_rule,
            commands::save_settings,
            commands::clear_recents,
            commands::set_default,
            commands::is_default,
            commands::set_launch_at_login,
            commands::reset_all,
            commands::open_settings,
            commands::open_external,
            commands::browser_icons,
            commands::uninstall_app,
            commands::update_status,
            commands::check_update,
            commands::install_update,
        ])
        .setup(move |app| {
            #[cfg(target_os = "macos")]
            app.set_activation_policy(tauri::ActivationPolicy::Accessory);
            let handle = app.handle();
            if !e2e {
                tray::setup(handle)?;
                updates::start_background_checks(handle);
            }
            // Pre-create the picker so it appears instantly when the first link arrives.
            app_windows::picker(handle);
            #[cfg(not(target_os = "macos"))]
            handle_args(handle, &args, true);
            #[cfg(target_os = "macos")]
            if has_flag(&args, "--settings") || !has_flag(&args, "--background") {
                show_settings_unless_link_arrives(handle);
            }
            Ok(())
        })
        .build(tauri::generate_context!())
        .expect("error while building PickMyBrowser");

    app.run(|_app, event| match event {
        #[cfg(target_os = "macos")]
        RunEvent::Opened { urls } => {
            for url in urls {
                if let Some(url) = url_from_arg(url.as_str()) {
                    open_in_picker(_app, &url);
                }
            }
        }
        RunEvent::ExitRequested { api, code: None, .. } => {
            // Keep running in the tray when windows close; an explicit quit sets an exit code.
            api.prevent_exit();
        }
        _ => {}
    });
}
